"""A small GitHub-flavoured Markdown renderer.

Written rather than taken from a library for two reasons. The mirror's CI pulls
this directory on its own, so every file it needs has to travel with it; and
GitHub's own renderer emits its class names, which do not match this site's
stylesheet — its tables would come out unstyled.

It covers what the camareLOL docs actually use: headings, fenced code, pipe
tables, blockquotes, lists, rules, and the usual inline marks. Anything it does
not understand is escaped and shown as text rather than silently dropped, so a
missing feature reads as plain text instead of disappearing.
"""

from __future__ import annotations

import html
import re

_HEADING = re.compile(r"^(#{1,6})\s+(.*?)\s*#*$")
_FENCE = re.compile(r"^\s*(```+|~~~+)\s*([\w+-]*)\s*$")
_RULE = re.compile(r"^\s*(?:-{3,}|\*{3,}|_{3,})\s*$")
_UL = re.compile(r"^(\s*)[-*+]\s+(.*)$")
_OL = re.compile(r"^(\s*)(\d+)[.)]\s+(.*)$")
_TABLE_SEP = re.compile(r"^\s*\|?\s*:?-{1,}:?\s*(\|\s*:?-{1,}:?\s*)+\|?\s*$")
_LINK = re.compile(r"\[([^\]]*)\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)")
_IMAGE = re.compile(r"!\[([^\]]*)\]\(([^)\s]+)\)")
_CODE = re.compile(r"`([^`]+)`")
_BOLD = re.compile(r"\*\*([^*]+)\*\*")
_BOLD_ALT = re.compile(r"__([^_]+)__")
_ITALIC = re.compile(r"(?<![\w*])\*([^*\n]+)\*(?![\w*])")
_AUTOLINK = re.compile(r"<(https?://[^>\s]+)>")
_BARE_URL = re.compile(r"(?<![\"'=>])\b(https?://[^\s<>()\[\]]+[^\s<>()\[\].,;:!?])")


def escape(text: str) -> str:
    return html.escape(text, quote=False)


def safe_url(url: str) -> str:
    """Only http, https and in-page anchors.

    The docs are ours, but a page that renders whatever a file contains should
    not be able to carry `javascript:` through into a link just because
    somebody pasted one. Anything else is rendered as inert text.
    """
    candidate = url.strip()
    lowered = candidate.lower()
    if lowered.startswith(("http://", "https://", "#", "/")) or (
        lowered.startswith("mailto:") or lowered.startswith("./") or lowered.startswith("../")
    ):
        return escape(candidate)
    return "#"


def inline(text: str) -> str:
    """Inline marks, with the text already escaped."""
    placeholders: list[str] = []

    def stash(fragment: str) -> str:
        placeholders.append(fragment)
        return f"\x00{len(placeholders) - 1}\x00"

    # Code first: nothing inside backticks is markup.
    text = _CODE.sub(lambda m: stash(f"<code>{escape(m.group(1))}</code>"), text)
    text = _IMAGE.sub(
        lambda m: stash(
            f'<img src="{safe_url(m.group(2))}" alt="{escape(m.group(1))}" loading="lazy">'
        ),
        text,
    )
    text = _LINK.sub(
        lambda m: stash(
            f'<a href="{safe_url(m.group(2))}">{escape(m.group(1)) or escape(m.group(2))}</a>'
        ),
        text,
    )
    text = _AUTOLINK.sub(lambda m: stash(f'<a href="{escape(m.group(1))}">{escape(m.group(1))}</a>'), text)

    text = _BARE_URL.sub(
        lambda m: stash(f'<a href="{escape(m.group(1))}">{escape(m.group(1))}</a>'), text
    )

    # Escape first, then emphasise. Doing it the other way round inserts the
    # <strong> tags into text that escape() then hides, and `**x**` comes out
    # as literal &lt;strong&gt;.
    out = escape(text)
    out = _BOLD.sub(lambda m: f"<strong>{m.group(1)}</strong>", out)
    out = _BOLD_ALT.sub(lambda m: f"<strong>{m.group(1)}</strong>", out)
    out = _ITALIC.sub(lambda m: f"<em>{m.group(1)}</em>", out)
    for i, fragment in enumerate(placeholders):
        out = out.replace(f"\x00{i}\x00", fragment)
    return out


def _split_row(line: str) -> list[str]:
    line = line.strip()
    if line.startswith("|"):
        line = line[1:]
    if line.endswith("|"):
        line = line[:-1]
    return [cell.strip() for cell in line.split("|")]


def render(markdown: str) -> str:
    lines = markdown.replace("\r\n", "\n").split("\n")
    out: list[str] = []
    i = 0

    while i < len(lines):
        line = lines[i]

        # fenced code
        fence = _FENCE.match(line)
        if fence:
            marker = fence.group(1)[0] * 3
            lang = fence.group(2)
            i += 1
            body: list[str] = []
            while i < len(lines) and not lines[i].strip().startswith(marker):
                body.append(lines[i])
                i += 1
            i += 1
            cls = f' class="language-{escape(lang)}"' if lang else ""
            out.append(f"<pre><code{cls}>{escape(chr(10).join(body))}</code></pre>")
            continue

        if not line.strip():
            i += 1
            continue

        if _RULE.match(line):
            out.append("<hr>")
            i += 1
            continue

        heading = _HEADING.match(line)
        if heading:
            level = len(heading.group(1))
            out.append(f"<h{level}>{inline(heading.group(2))}</h{level}>")
            i += 1
            continue

        # pipe table: a header row followed by a separator row
        if "|" in line and i + 1 < len(lines) and _TABLE_SEP.match(lines[i + 1]):
            header = _split_row(line)
            i += 2
            rows: list[list[str]] = []
            while i < len(lines) and "|" in lines[i] and lines[i].strip():
                rows.append(_split_row(lines[i]))
                i += 1
            out.append("<table><thead><tr>")
            out += [f"<th>{inline(c)}</th>" for c in header]
            out.append("</tr></thead><tbody>")
            for row in rows:
                cells = row + [""] * (len(header) - len(row))
                out.append("<tr>" + "".join(f"<td>{inline(c)}</td>" for c in cells[: len(header)]) + "</tr>")
            out.append("</tbody></table>")
            continue

        if line.lstrip().startswith(">"):
            quoted: list[str] = []
            while i < len(lines) and lines[i].lstrip().startswith(">"):
                quoted.append(lines[i].lstrip().lstrip(">").strip())
                i += 1
            out.append(f"<blockquote>{render(chr(10).join(quoted))}</blockquote>")
            continue

        if _UL.match(line) or _OL.match(line):
            ordered = bool(_OL.match(line))
            tag = "ol" if ordered else "ul"
            pattern = _OL if ordered else _UL
            items: list[str] = []
            while i < len(lines):
                m = pattern.match(lines[i])
                if not m:
                    if lines[i].strip() and lines[i].startswith(("  ", "\t")) and items:
                        items[-1] += "\n" + lines[i].strip()
                    else:
                        break
                    i += 1
                    continue
                body = m.group(3) if ordered else m.group(2)
                # a task list checkbox
                check = ""
                cb = re.match(r"\[([ xX])\]\s*(.*)$", body)
                if cb:
                    done = " checked" if cb.group(1).lower() == "x" else ""
                    check = f'<input type="checkbox" disabled{done}> '
                    body = cb.group(2)
                items.append(f"<li>{check}{inline(body)}</li>")
                i += 1
            out.append(f"<{tag}>" + "".join(items) + f"</{tag}>")
            continue

        # paragraph: consume until a blank line or a block starter
        para: list[str] = []
        while i < len(lines) and lines[i].strip():
            nxt = lines[i]
            if para and (
                _HEADING.match(nxt)
                or _FENCE.match(nxt)
                or _RULE.match(nxt)
                or _UL.match(nxt)
                or _OL.match(nxt)
                or nxt.lstrip().startswith(">")
            ):
                break
            para.append(nxt.strip())
            i += 1
        if para:
            out.append("<p>" + "<br>".join(inline(p) for p in para) + "</p>")

    return "\n".join(out)