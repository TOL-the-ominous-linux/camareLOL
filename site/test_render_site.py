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
import re
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

    def stub_repository(self, rels=None, docs=("index.md", "testing.md")):
        """Answer both API shapes the build asks for."""
        rels = TWO_RELEASES if rels is None else rels

        def fake_get(url: str):
            if "releases" in url:
                return rels
            if "/contents/docs/" in url:
                # The build lists one language directory at a time.
                return [{"name": n} for n in docs]
            raise AssertionError(f"unexpected call: {url}")

        render_site.get_json = fake_get
        render_site.fetch_source = lambda path: f"# {Path(path).stem}\n\nNội dung.\n"

    def run_build(self, docs: str | None = None) -> int:
        # --docs points at a temporary tree by default. The build reads
        # documentation from disk when it is there, so without this the
        # repository's real docs/ would be rendered instead of the stub, and
        # the tests below would pass for the wrong reason.
        docs_dir = Path(docs) if docs else self.out / "nodocs"
        argv = sys.argv
        sys.argv = ["render_site.py", "--out", str(self.out),
                    "--status", str(self.out / "s.json"), "--docs", str(docs_dir)]
        try:
            return render_site.main()
        finally:
            sys.argv = argv

    def write_docs(self, lang: str, names=("index.md", "testing.md")) -> None:
        d = self.out / "nodocs" / lang
        d.mkdir(parents=True, exist_ok=True)
        for n in names:
            (d / n).write_text(f"# {Path(n).stem}\n\nNội dung.\n", encoding="utf-8")

    def write_docs_for_every_language(self, names=("index.md", "testing.md")) -> None:
        for lang in render_site.LANGS:
            self.write_docs(lang, names)


class TestDocumentsLinkToFilesThatExist(unittest.TestCase):
    """A relative link in a document must resolve on GitHub and on the site.

    Both are checked here rather than trusted: the path is correct on GitHub
    only if the file is really there, and correct on the site only if it is
    rewritten to the page that file became — in the language of the page it
    appears on.
    """

    def files(self):
        docs = Path(__file__).resolve().parent.parent / "docs"
        found = []
        for lang_dir in sorted(p for p in docs.iterdir() if p.is_dir()):
            found += sorted(lang_dir.glob("*.md"))
        return found

    def targets(self, path):
        return [t for t in re.findall(r"\]\(([^)#\s]+\.md)(?:#[^)]*)?\)",
                                      path.read_text(encoding="utf-8"))
                if not t.startswith(("http://", "https://"))]

    def test_every_relative_markdown_link_resolves(self):
        broken = []
        for path in self.files():
            for target in self.targets(path):
                if not (path.parent / target).resolve().exists():
                    broken.append(f"{path.parent.name}/{path.name} -> {target}")
        self.assertEqual(broken, [], "links to files that are not there")

    def test_every_one_of_them_becomes_a_page_that_gets_written(self):
        for path in self.files():
            lang = path.parent.name
            for target in self.targets(path):
                page = render_site.rewrite_doc_link(target, lang)
                self.assertTrue(page.startswith(f"/{lang}/docs/"), f"{target} -> {page}")
                self.assertTrue(page.endswith(".html"), f"{target} -> {page}")


class TestDocLinksPointAtRealPages(unittest.TestCase):
    """A link to a .md file is right on GitHub and broken on the site.

    The documents link to each other by repository path, and the site does not
    serve Markdown — only the pages it was rendered into.
    """

    def test_a_docs_path_becomes_the_rendered_page(self):
        self.assertEqual(render_site.rewrite_doc_link("docs/testing.md", "vi"),
                         "/vi/docs/testing.html")

    def test_a_sibling_link_from_inside_docs_works_too(self):
        # A page links to its neighbour with no directory part at all.
        self.assertEqual(render_site.rewrite_doc_link("ipc-protocol.md", "en"),
                         "/en/docs/ipc-protocol.html")

    def test_the_index_link_becomes_a_page(self):
        self.assertEqual(render_site.rewrite_doc_link("index.md", "zh"),
                         "/zh/docs/index.html")

    def test_the_link_keeps_the_language_of_the_page_it_is_on(self):
        """A reader who arrived in Vietnamese must not land in English mid-page."""
        for lang in render_site.LANGS:
            got = render_site.rewrite_doc_link("testing.md", lang)
            self.assertTrue(got.startswith(f"/{lang}/docs/"), f"{lang}: {got}")

    def test_a_fragment_is_kept(self):
        self.assertEqual(render_site.rewrite_doc_link("docs/testing.md#vm", "en"),
                         "/en/docs/testing.html#vm")

    def test_absolute_and_external_links_are_left_alone(self):
        for url in ("https://e.invalid/x.md", "/docs/a.html", "#top", "mailto:a@e.invalid"):
            self.assertEqual(render_site.rewrite_doc_link(url, "en"), url)

    def test_a_link_that_is_not_markdown_is_left_alone(self):
        self.assertEqual(render_site.rewrite_doc_link("docs/x.txt", "en"), "docs/x.txt")

    def test_the_rewritten_target_is_the_case_the_files_are_written_in(self):
        """The site writes them lower case; a mixed-case link would be a redirect."""
        self.assertNotIn(".MD", render_site.rewrite_doc_link("docs/TESTING.MD", "en"))


class TestEveryLanguageHasTheSamePages(unittest.TestCase):
    def setUp(self):
        self.docs = Path(__file__).resolve().parent.parent / "docs"

    def languages(self):
        return sorted(p.name for p in self.docs.iterdir() if p.is_dir())

    def test_at_least_three_languages_exist(self):
        self.assertGreaterEqual(len(self.languages()), 3, self.languages())

    def test_every_language_has_the_same_set_of_pages(self):
        """A reader who switches language must find the same page, not a gap.

        A missing translation is worse than none: the language switcher offers
        a language, the reader picks it, and the page they were reading simply
        stops existing.
        """
        sets = {lang: {p.name for p in (self.docs / lang).glob("*.md")}
                for lang in self.languages()}
        base = sets[self.languages()[0]]
        for lang, names in sets.items():
            self.assertEqual(names, base, f"{lang} differs: "
                             f"missing {base - names}, extra {names - base}")

    def test_every_page_links_only_to_pages_that_exist(self):
        for lang in self.languages():
            for md in sorted((self.docs / lang).glob("*.md")):
                for target in re.findall(r"\]\(([^)#\s]+\.md)\)", md.read_text(encoding="utf-8")):
                    if target.startswith(("http://", "https://")):
                        continue
                    resolved = (md.parent / target).resolve()
                    self.assertTrue(resolved.exists(),
                                    f"{lang}/{md.name} -> {target} does not exist")


class TestRefusesToPublishIncompleteSites(BuildCase):
    def test_no_releases_means_failure(self):
        render_site.get_json = offline
        render_site.fetch_source = offline
        self.put_mirror()
        self.assertEqual(self.run_build(), 1)

    def test_no_docs_means_failure(self):
        """A language with no pages anywhere stops the build.

        The switcher offers that language on every page, so a reader who picks
        it must not land on an empty list. Both sources have to be empty:
        the build prefers the working tree and falls back to the API, so
        removing only the directory would still find them there.
        """
        import shutil
        self.stub_repository()
        self.put_mirror()
        for lang in render_site.LANGS:
            self.write_docs(lang)
        shutil.rmtree(self.out / "nodocs" / "zh")

        saved = render_site.get_json
        render_site.get_json = lambda url: [] if "/contents/docs/zh" in url else saved(url)
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
        self.write_docs_for_every_language()
        self.assertEqual(self.run_build(), 1)

    def test_a_listing_entry_without_a_name_does_not_abort(self):
        """A listing entry with no usable name is skipped, not fatal."""
        self.stub_repository(docs=("updates-signing.md", "no-name-entry"))
        self.put_mirror()
        self.write_docs_for_every_language(("updates-signing.md",))
        self.assertEqual(self.run_build(), 0)
        self.assertTrue((self.out / "vi" / "docs" / "updates-signing.html").exists())


class TestBuildsWhenItHasWhatItNeeds(BuildCase):
    def test_succeeds_and_writes_every_page(self):
        self.stub_repository()
        self.put_mirror()
        self.write_docs_for_every_language()
        self.assertEqual(self.run_build(), 0)
        for name in ("index.html", "news.html", "status.html", "docs.html", "discuss.html",
                     "assets/style.css", "assets/logo.png",
                     "en/docs/index.html", "vi/docs/index.html", "zh/docs/index.html",
                     "en/docs/testing.html", "vi/docs/testing.html", "zh/docs/testing.html"):
            self.assertTrue((self.out / name).exists(), f"missing {name}")

    def test_every_language_gets_its_own_directory(self):
        self.stub_repository()
        self.put_mirror()
        self.write_docs_for_every_language()
        self.run_build()
        for lang in render_site.LANGS:
            self.assertTrue((self.out / lang / "docs" / "index.html").exists(), lang)

    def test_doc_links_match_the_generated_file_names(self):
        """A link in the wrong case is a redirect, not a page.

        The static host normalises /docs/UPDATES.html into /docs/updates, so a
        link written in the original case only works by accident.
        """
        self.stub_repository()
        self.put_mirror()
        self.write_docs_for_every_language()
        self.run_build()
        listed = (self.out / "docs.html").read_text(encoding="utf-8")
        for lang in render_site.LANGS:
            base = f"/{lang}/docs"
            self.assertIn(f'href="{base}/"', listed)
            for name in (f.stem for f in (self.out / lang / "docs").glob("*.html")):
                self.assertNotIn(f'href="{base}/{name.upper()}.html"', listed)

    def test_the_language_switcher_marks_the_current_one(self):
        """Three identical links are not a switch; a reader cannot tell where
        they are."""
        self.stub_repository()
        self.put_mirror()
        self.write_docs_for_every_language()
        self.run_build()
        for lang in render_site.LANGS:
            body = (self.out / lang / "docs" / "index.html").read_text(encoding="utf-8")
            self.assertIn('<html lang="%s"' % lang, body)
            self.assertIn('class="on"', body)

    def test_the_mirror_files_are_left_exactly_as_they_were(self):
        self.stub_repository()
        self.put_mirror()
        self.write_docs_for_every_language()
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