#!/usr/bin/env python3
"""Tests for the site build.

    python site/test_render_site.py

The network is stubbed out. What is being checked is the build's behaviour when
the repository cannot be read — which happens for real, because GitHub rate
limits unauthenticated calls — and that it fails loudly rather than publishing
a site that looks fine and says nothing.
"""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
import urllib.error
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import render_site  # noqa: E402


def offline(*_args, **_kwargs):
    raise urllib.error.URLError("network is down")


TWO_RELEASES = [
    {
        "tag_name": "v0.2.0",
        "name": "Bản mới",
        "body": "Sửa lỗi.",
        "published_at": "2026-10-01T00:00:00Z",
        "html_url": "https://example.invalid/r",
    },
    {
        "tag_name": "v0.1.9",
        "name": "Bản cũ",
        "body": "Cũ.",
        "published_at": "2026-09-01T00:00:00Z",
        "html_url": "https://example.invalid/r",
    },
]


class BuildCase(unittest.TestCase):
    """A build into a temporary directory, with the network replaced."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.out = Path(self.tmp.name)
        self.addCleanup(self.tmp.cleanup)
        self._saved = (render_site.get_json, render_site.fetch_source, render_site.render_status)
        render_site.render_status = lambda _p: "<p>ok</p>"
        self.addCleanup(self._restore)

    def _restore(self):
        render_site.get_json, render_site.fetch_source, render_site.render_status = self._saved

    def put_mirror(self, signature: bool = True):
        (self.out / "latest.json").write_text(json.dumps({"version": "9.9.9"}))
        if signature:
            (self.out / "latest.json.sig").write_text("SIGNATURE")

    def stub_repository(self, rels=None, docs=("UPDATES.md", "README.md")):
        """Answer both API shapes the build asks for."""
        rels = TWO_RELEASES if rels is None else rels

        def fake_get(url: str):
            if "releases" in url:
                return rels
            if "/contents/docs" in url:
                return [{"name": n} for n in docs]
            raise AssertionError(f"unexpected call: {url}")

        render_site.get_json = fake_get
        render_site.fetch_source = lambda path: f"# {Path(path).stem}\n\nNội dung.\n"

    def run_build(self) -> int:
        argv = sys.argv
        sys.argv = ["render_site.py", "--out", str(self.out), "--status", str(self.out / "s.json")]
        try:
            return render_site.main()
        finally:
            sys.argv = argv


class TestDocLinksPointAtRealPages(unittest.TestCase):
    """A link to a .md file is right on GitHub and broken on the site.

    The documents link to each other by repository path, and the site does not
    serve Markdown — only the pages it was rendered into.
    """

    def test_a_docs_path_becomes_the_rendered_page(self):
        self.assertEqual(render_site.rewrite_doc_link("docs/UPDATES.md"),
                         "/docs/updates.html")

    def test_a_sibling_link_from_inside_docs_works_too(self):
        # docs/ARCHITECTURE.md links to IPC-PROTOCOL.md with no directory part.
        self.assertEqual(render_site.rewrite_doc_link("IPC-PROTOCOL.md"),
                         "/docs/ipc-protocol.html")

    def test_the_readme_link_becomes_a_page(self):
        self.assertEqual(render_site.rewrite_doc_link("README.md"),
                         "/docs/readme.html")

    def test_a_fragment_is_kept(self):
        self.assertEqual(render_site.rewrite_doc_link("docs/TESTING.md#vm"),
                         "/docs/testing.html#vm")

    def test_absolute_and_external_links_are_left_alone(self):
        for url in ("https://e.invalid/x.md", "/docs/a.html", "#top", "mailto:a@e.invalid"):
            self.assertEqual(render_site.rewrite_doc_link(url), url)

    def test_a_link_that_is_not_markdown_is_left_alone(self):
        self.assertEqual(render_site.rewrite_doc_link("docs/x.txt"), "docs/x.txt")

    def test_the_rewritten_target_is_the_case_the_files_are_written_in(self):
        """The site writes them lower case; a mixed-case link would be a redirect."""
        self.assertNotIn(".MD", render_site.rewrite_doc_link("docs/UPDATES.MD"))


class TestRefusesToPublishIncompleteSites(BuildCase):
    def test_no_releases_means_failure(self):
        render_site.get_json = offline
        render_site.fetch_source = offline
        self.put_mirror()
        self.assertEqual(self.run_build(), 1)

    def test_no_docs_means_failure(self):
        self.stub_repository(docs=())
        render_site.fetch_source = lambda _p: None
        self.put_mirror()
        self.assertEqual(self.run_build(), 1)

    def test_a_missing_mirror_file_means_failure(self):
        """The pages and the index share a directory.

        Publishing the pages while the signature is absent would replace the
        update URL every camareLOL uses with a 404.
        """
        self.stub_repository()
        self.put_mirror(signature=False)
        self.assertEqual(self.run_build(), 1)

    def test_an_unusable_release_entry_means_failure_not_a_crash(self):
        """A response entry with no tag is noise; the build should say so."""
        self.stub_repository(rels=[{"name": "no tag here"}, "not even an object"])
        self.put_mirror()
        self.assertEqual(self.run_build(), 1)

    def test_a_listing_entry_without_a_name_does_not_abort(self):
        self.stub_repository(docs=("UPDATES.md", "no-name-entry"))
        self.put_mirror()
        self.assertEqual(self.run_build(), 0)
        self.assertTrue((self.out / "docs" / "updates.html").exists())


class TestBuildsWhenItHasWhatItNeeds(BuildCase):
    def test_succeeds_and_writes_every_page(self):
        self.stub_repository()
        self.put_mirror()
        self.assertEqual(self.run_build(), 0)
        for name in ("index.html", "news.html", "status.html", "docs.html", "discuss.html",
                     "assets/style.css", "assets/logo.png", "docs/updates.html",
                     "docs/readme.html"):
            self.assertTrue((self.out / name).exists(), f"missing {name}")

    def test_doc_links_match_the_generated_file_names(self):
        """A link in the wrong case is a redirect, not a page.

        The static host normalises /docs/UPDATES.html into /docs/updates, so a
        link written in the original case only works by accident.
        """
        self.stub_repository()
        self.put_mirror()
        self.run_build()
        listed = (self.out / "docs.html").read_text(encoding="utf-8")
        for name in (f.stem for f in (self.out / "docs").glob("*.html")):
            self.assertIn(f'href="/docs/{name}.html"', listed)
            self.assertNotIn(f'href="/docs/{name.upper()}.html"', listed)

    def test_the_mirror_files_are_left_exactly_as_they_were(self):
        self.stub_repository()
        self.put_mirror()
        before = ((self.out / "latest.json").read_bytes(),
                  (self.out / "latest.json.sig").read_bytes())
        self.run_build()
        self.assertEqual(before[0], (self.out / "latest.json").read_bytes())
        self.assertEqual(before[1], (self.out / "latest.json.sig").read_bytes())

    def test_release_text_cannot_inject_markup(self):
        self.stub_repository(rels=[dict(TWO_RELEASES[0],
                                        name="<script>alert(1)</script>",
                                        body="<img onerror=alert(1)>")])
        self.put_mirror()
        self.assertEqual(self.run_build(), 0)
        news = (self.out / "news.html").read_text(encoding="utf-8")
        # The payload must survive as visible text, not as markup. Searching for
        # "onerror=" alone would also reject the escaped form, which is harmless.
        self.assertIn("&lt;img onerror=alert(1)&gt;", news)
        self.assertNotIn("<script>", news)
        self.assertNotIn("<img onerror", news)


if __name__ == "__main__":
    unittest.main(verbosity=2)