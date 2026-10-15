#!/usr/bin/env python3
"""Tests for the site's Markdown renderer.

    python site/test_markdown.py
"""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from markdown import render  # noqa: E402


class TestHeadings(unittest.TestCase):
    def test_levels(self):
        self.assertIn("<h1>Title</h1>", render("# Title"))
        self.assertIn("<h3>Deep</h3>", render("### Deep"))

    def test_closing_hashes_are_dropped(self):
        self.assertIn("<h2>Name</h2>", render("## Name ##"))

    def test_marks_inside_a_heading_work(self):
        self.assertIn("<code>ls</code>", render("# Run `ls`"))


class TestCode(unittest.TestCase):
    def test_fence_keeps_language_for_highlighting(self):
        out = render("```rust\nlet x = 1;\n```")
        self.assertIn('class="language-rust"', out)
        self.assertIn("let x = 1;", out)

    def test_markup_inside_a_fence_is_not_markup(self):
        # The whole point of a fence: **stars** stay literal.
        self.assertIn("**stars**", render("```\n**stars**\n```"))
        self.assertNotIn("<strong>", render("```\n**stars**\n```"))

    def test_html_inside_a_fence_is_escaped(self):
        self.assertIn("&lt;script&gt;", render("```\n<script>\n```"))


class TestInline(unittest.TestCase):
    def test_code_span_beats_everything_inside_it(self):
        self.assertIn("<code>**not bold**</code>", render("`**not bold**`"))

    def test_bold_and_italic(self):
        self.assertIn("<strong>x</strong>", render("**x**"))
        self.assertIn("<em>x</em>", render("*x*"))

    def test_link(self):
        self.assertIn('<a href="https://e.invalid">text</a>', render("[text](https://e.invalid)"))

    def test_bare_url_becomes_a_link(self):
        self.assertIn('<a href="https://e.invalid">https://e.invalid</a>', render("see https://e.invalid"))

    def test_url_inside_a_code_span_stays_a_link_free_code_span(self):
        out = render("`https://e.invalid`")
        self.assertIn("<code>https://e.invalid</code>", out)
        self.assertNotIn("<a href", out)

    def test_html_in_prose_is_escaped(self):
        # A docs file must not be able to inject script into the page.
        self.assertIn("&lt;script&gt;", render("hello <script>alert(1)</script>"))
        self.assertNotIn("<script>", render("hello <script>alert(1)</script>"))

    def test_a_dangerous_link_scheme_is_neutralised(self):
        # Docs are ours, but a page that renders whatever a file contains must
        # not pass a javascript: URL through into an href.
        out = render("[click](javascript:alert(1))")
        self.assertNotIn('href="javascript:', out)

    def test_relative_and_anchor_links_survive(self):
        self.assertIn('href="#abc"', render("[x](#abc)"))
        self.assertIn('href="/docs/a.html"', render("[x](/docs/a.html)"))

    def test_a_plain_relative_path_stays_a_link(self):
        """Not '#'.

        An earlier allow-list turned every ordinary relative path into href="#".
        The README's own links — [docs/UPDATES.md](docs/UPDATES.md) and five
        more — rendered as links that looked real and went nowhere.
        """
        out = render("[docs/UPDATES.md](docs/UPDATES.md)")
        self.assertIn('href="docs/UPDATES.md"', out)
        self.assertNotIn('href="#"', out)

    def test_a_relative_path_inside_a_table_cell_survives(self):
        self.assertNotIn('href="#"', render("| a |\n|---|\n| [x](y/z.md) |"))

    def test_a_relative_path_inside_a_list_survives(self):
        self.assertNotIn('href="#"', render("- [see](docs/TESTING.md)"))

    def test_a_heading_link_survives(self):
        self.assertNotIn('href="#"', render("# See [x](docs/TESTING.md)"))


class TestRewriteHook(unittest.TestCase):
    def test_targets_are_passed_through_the_hook(self):
        out = render("[x](docs/UPDATES.md)", rewrite=lambda u: "/docs/updates.html")
        self.assertIn('href="/docs/updates.html"', out)

    def test_the_hook_does_not_touch_code_spans(self):
        out = render("`docs/UPDATES.md`", rewrite=lambda u: "/nope.html")
        self.assertIn("<code>docs/UPDATES.md</code>", out)
        self.assertNotIn("/nope.html", out)


class TestTables(unittest.TestCase):
    def test_header_and_rows(self):
        out = render("| a | b |\n|---|---|\n| 1 | 2 |")
        self.assertIn("<th>a</th>", out)
        self.assertIn("<td>1</td>", out)
        self.assertIn("<td>2</td>", out)

    def test_alignment_markers_are_consumed_not_shown(self):
        out = render("| a | b |\n|:--|--:|\n| 1 | 2 |")
        self.assertNotIn("--:", out)

    def test_a_short_row_is_padded_not_crashed(self):
        out = render("| a | b | c |\n|---|---|---|\n| 1 |")
        self.assertIn("<td></td>", out)


class TestBlocks(unittest.TestCase):
    def test_quote(self):
        self.assertIn("<blockquote>", render("> careful"))
        self.assertIn("careful", render("> careful"))

    def test_unordered_list(self):
        self.assertIn("<li>a</li>", render("- a\n- b"))

    def test_ordered_list(self):
        self.assertIn("<ol><li>a</li>", render("1. a\n2. b"))

    def test_task_list(self):
        out = render("- [x] done\n- [ ] todo")
        self.assertIn('type="checkbox" disabled checked', out)
        self.assertIn('type="checkbox" disabled>', out)

    def test_rule(self):
        self.assertIn("<hr>", render("---"))

    def test_a_paragraph_stops_at_the_next_block(self):
        # Otherwise "text" and the heading after it get glued into one <p>.
        out = render("intro\n\n# Title")
        self.assertIn("<p>intro</p>", out)
        self.assertIn("<h1>Title</h1>", out)


class TestRealDocs(unittest.TestCase):
    """The actual files, because the unit cases only prove what I imagined."""

    def test_repository_docs_render(self):
        docs = Path(__file__).resolve().parent.parent / "docs"
        if not docs.is_dir():
            self.skipTest("no docs directory")
        rendered = 0
        for md in sorted(docs.glob("*.md")):
            out = render(md.read_text(encoding="utf-8"))
            self.assertNotIn("&lt;h2&gt;", out, f"{md.name}: headings left unrendered")
            self.assertTrue(out.strip(), f"{md.name}: rendered to nothing")
            self.assertNotIn("<script>", out, f"{md.name}: injected a script tag")
            rendered += 1
        self.assertGreater(rendered, 0, "no docs found to test")


if __name__ == "__main__":
    unittest.main(verbosity=2)