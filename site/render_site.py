#!/usr/bin/env python3
"""Build the camareLOL site: documentation and release news.

    python tools/render_site.py --out .

Everything is generated from the camareLOL repository, so the site cannot quietly
disagree with the code. Markdown is rendered by GitHub's own GFM renderer rather
than a local parser: no dependency to keep current, and the result is exactly
what people already read on GitHub.

This writes into the repository root because that is what GitHub Pages publishes.
"""

from __future__ import annotations

import argparse
import base64
import html
import json
import os
import sys
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import markdown  # noqa: E402

OWNER = "tamkogay9876-code"
REPO = "camareLOL"
API = f"https://api.github.com/repos/{OWNER}/{REPO}"
BRANCH = "main"
TIMEOUT = 30

SITE_NAME = "camareLOL"

def log(msg: str) -> None:
    print(f"[site] {msg}", flush=True)


def _request(url: str, data: bytes | None = None, accept: str = "application/vnd.github+json") -> urllib.request.Request:
    headers = {"Accept": accept, "User-Agent": "camareLOL-site/1.0"}
    if data is not None:
        headers["Content-Type"] = "application/json"
    token = os.environ.get("GITHUB_TOKEN")
    if token:
        headers["Authorization"] = f"Bearer {token}"
    return urllib.request.Request(url, data=data, headers=headers)


def get_json(url: str):
    with urllib.request.urlopen(_request(url), timeout=TIMEOUT) as resp:
        return json.load(resp)


def fetch_source(path: str) -> str | None:
    """Read a file from the camareLOL repository."""
    url = f"{API}/contents/{path}?ref={BRANCH}"
    try:
        doc = get_json(url)
    except (urllib.error.URLError, OSError, json.JSONDecodeError):
        return None
    return base64.b64decode(doc["content"]).decode("utf-8", "replace")


def rewrite_doc_link(url: str, lang: str = "en") -> str:
    """Point a link at a Markdown file to the page that file became.

    The documents link to each other by their repository paths — `testing.md`
    from the index page, `ipc-protocol.md` from the architecture notes — and
    those links are right on GitHub. The site does not serve the Markdown, so
    left alone they resolve to a path that is not there.

    Two details that are easy to get wrong and produce a page that looks fine:

    - the target keeps the language of the page it appears on, so a reader who
      arrived in Vietnamese does not land in English halfway through;
    - pages are written in lower case, because the static host normalises a
      mixed-case name into a redirect.
    """
    if url.startswith(("http://", "https://", "#", "mailto:", "/")):
        return url
    base, _, fragment = url.partition("#")
    lowered = base.lower()
    if not lowered.endswith(".md"):
        return url
    stem = lowered[:-3]
    stem = stem[len("docs/"):] if stem.startswith("docs/") else stem
    return f"/{lang_dir(lang)}/{stem}.html" + (f"#{fragment}" if fragment else "")


def render_markdown(text: str, lang: str = "en") -> str:
    """Render locally rather than through the GitHub API.

    The API route is gone — /repos/{owner}/{repo}/markdown now answers 404 even
    with a valid token — and its output carries GitHub's class names, which do
    not match this site's stylesheet, so the tables would arrive unstyled.
    """
    return markdown.render(text, rewrite=lambda u: rewrite_doc_link(u, lang))


NAV = [
    ("index.html", "Trang chủ"),
    ("news.html", "Cập nhật"),
    ("docs.html", "Tài liệu"),
    ("discuss.html", "Thảo luận"),
]


def page(title: str, body: str, active: str, lang: str = "vi") -> str:
    nav_items = "\n".join(
        '      <a href="/{href}"{on}>{label}</a>'.format(
            href=href, label=label, on=' class="on"' if href == active else ""
        )
        for href, label in NAV
    )
    switcher = "".join(
        '        <a href="/{href}"{on}>{label}</a>'.format(
            href=lang_dir(code),
            label=html.escape(name),
            on=' class="on"' if code == lang else "",
        )
        for code, name in sorted(LANGS.items())
    )
    # html lang must match the document, or a screen reader is told to read
    # Chinese text with Vietnamese pronunciation rules.
    return f"""<!doctype html>
<html lang="{html.escape(lang)}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{html.escape(title)} — {SITE_NAME}</title>
<link rel="stylesheet" href="/assets/style.css">
<link rel="icon" href="/assets/logo.png">
<meta name="description" content="camareLOL — trình cài đặt Arch Linux an toàn, cập nhật có chữ ký.">
</head>
<body>
<header>
  <div class="wrap">
    <a class="brand" href="/index.html">
      <img src="/assets/logo.png" alt="" width="40" height="40">
      <span>camare<strong>LOL</strong></span>
    </a>
    <nav>
{nav_items}
    </nav>
  </div>
</header>
<main class="wrap">
{body}
</main>
<footer class="wrap">
  <nav class="langs" aria-label="Language">
{switcher}
  </nav>
  <p>camareLOL là trình cài đặt Arch Linux dựa trên Rust. Mọi bản cập nhật đều
     được ký bằng GPG và kiểm tra bằng khoá nhúng trong chính binary — kể cả khi
     tải qua mirror này.</p>
  <p><a href="https://github.com/{OWNER}/{REPO}">mã nguồn</a> ·
     <a href="https://github.com/{OWNER}/{REPO}/releases">phát hành</a> ·
     <a href="https://github.com/{OWNER}/{REPO}/discussions">thảo luận</a></p>
</footer>
</body>
</html>
"""


def releases() -> list[dict]:
    try:
        rels = get_json(f"{API}/releases?per_page=20")
    except (urllib.error.URLError, OSError, json.JSONDecodeError) as exc:
        log(f"could not read releases ({exc})")
        return []
    # An entry with no tag cannot be linked to, dated or labelled, so it is not
    # a release as far as this page is concerned.
    return [r for r in rels if isinstance(r, dict) and not r.get("draft") and r.get("tag_name")]


def fmt_size(n: int | None) -> str:
    if not n:
        return "—"
    if n < 1024:
        return f"{n} B"
    if n < 1024 * 1024:
        return f"{n / 1024:.0f} KB"
    return f"{n / (1024 * 1024):.1f} MB"


def render_news(rels: list[dict]) -> str:
    rows = []
    for r in rels:
        tag = html.escape(r["tag_name"])
        when = (r.get("published_at") or "")[:10]
        binary = next((a for a in r.get("assets", []) if a["name"] == "camarelol"), None)
        badge = ' <span class="tag pre">pre</span>' if r.get("prerelease") else ""
        dl = binary["browser_download_url"] if binary else "#"
        size = fmt_size(binary["size"] if binary else None)
        body = html.escape((r.get("body") or "").strip())
        rows.append(
            f'    <li>\n'
            f'      <span class="score">{size}</span>'
            f'<a href="https://github.com/{OWNER}/{REPO}/releases/tag/{tag}">{tag}</a>{badge}\n'
            f'      <div class="sub">{when} · <a href="{dl}">tải camarelol</a></div>\n'
            f'      <div class="note">{body[:200]}</div>\n'
            f"    </li>"
        )
    if not rows:
        rows.append('    <li><div class="note">chưa đọc được danh sách phát hành</div></li>')
    return "\n".join(rows)


def doc_list() -> str:
    """Links to each language's documentation index.

    Not a flat file list. The documentation now exists in three languages, and
    listing every page three times on one page would be unreadable and would
    imply the translations are page-for-page identical — they are not, and the
    entry pages differ by design.
    """
    items = "\n".join(
        f'    <li><a href="/{lang_dir(code)}/">{html.escape(name)}</a></li>'
        for code, name in sorted(LANGS.items())
    )
    return items


class Unpublishable(Exception):
    """The build cannot produce a site it is willing to put in front of anyone.

    Raised rather than logged-and-continued, because every way this can happen
    produces a page that looks finished and says nothing. It is caught in
    main() and turned into exit code 1, so it fails the step in CI the same way
    any other check does.
    """


#: Documentation languages, and how each is named in its own index page.
LANGS = {"en": "English", "vi": "Tiếng Việt", "zh": "中文"}


def lang_dir(lang: str) -> str:
    return f"{lang}/docs"


def write_doc_pages(out: Path, source_dir: Path | None = None) -> int:
    """Render every documentation page, one directory per language.

    Layout: `<lang>/docs/<page>.html`, so a page is at
    `https://tamkogay9876-code.github.io/camareLOL-mirror/vi/docs/updates-signing.html`.

    Language first, because the language is the thing that changes when a page
    does not. A `docs/vi/` prefix would bury the part that varies under the part
    that never does, and makes every future language a new branch of the tree
    rather than a new directory beside the existing ones.
    """
    count = 0
    for lang in sorted(LANGS):
        docs = out / lang_dir(lang)
        docs.mkdir(parents=True, exist_ok=True)

        local = source_dir / lang if source_dir else None
        if local and local.is_dir():
            names = sorted(p.name for p in local.glob("*.md"))
        else:
            try:
                listing = get_json(f"{API}/contents/docs/{lang}?ref={BRANCH}")
                names = sorted(f["name"] for f in listing
                               if str(f.get("name", "")).endswith(".md"))
            except (urllib.error.URLError, OSError, json.JSONDecodeError) as exc:
                log(f"could not list docs/{lang}: {exc}")
                raise Unpublishable(f"docs/{lang} could not be read")

        if not names:
            # The language switcher offers this language on every page. Landing
            # on one that has no content is worse than it not being offered.
            raise Unpublishable(f"docs/{lang} is empty")

        for name in names:
            if local and local.is_dir():
                text = (local / name).read_text(encoding="utf-8")
            else:
                text = fetch_source(f"docs/{lang}/{name}")
            if text is None:
                log(f"skipped {path}: could not read it")
                continue
            slug = Path(name).stem.lower()
            title = slug.replace("-", " ").replace("_", " ").capitalize()
            rendered = render_markdown(text, lang)
            (docs / f"{slug}.html").write_text(
                page(title, f"  <article>{rendered}</article>", "", lang=lang),
                encoding="utf-8",
            )
            # Only real content counts. An index of contents is not a page of
            # documentation, and letting it count here would make the "no docs
            # were rendered" check below impossible to reach.
            count += 1

        log(f"{lang}: {len(names)} pages")
        _write_lang_index(out, lang, names)
    return count


def _write_lang_index(out: Path, lang: str, names: list[str]) -> None:
    """A table of contents for one language."""
    # Relative on disk, absolute in the hrefs. Joining an out/ with a string
    # starting in "/" discards out/ entirely — the build wrote to /en/docs/.
    rel = lang_dir(lang)
    href_base = f"/{rel}/"
    items = "\n".join(
        f'    <li><a href="{href_base}{Path(n).stem.lower()}.html">{html.escape(n)}</a></li>'
        for n in names
    )
    others = " · ".join(
        f'<a href="/{lang_dir(o)}/">{html.escape(LANGS[o])}</a>'
        for o in sorted(LANGS) if o != lang
    )
    (out / rel / "index.html").write_text(
        page(
            LANGS[lang],
            f'  <h1>{html.escape(LANGS[lang])}</h1>\n'
            f'  <p class="note">Xem ngôn ngữ khác: {others}</p>\n'
            f'  <ul class="docs">\n{items}\n  </ul>',
            href_base + "index.html",
            lang=lang,
        ),
        encoding="utf-8",
    )


def copy_assets(out: Path) -> None:
    """Put the stylesheet and logo next to the pages.

    They have to be copied rather than assumed to be there: the pages link to
    /assets/style.css, and a build that generated every HTML file while leaving
    that path empty would publish an unstyled site and still look successful.
    """
    src = Path(__file__).resolve().parent / "assets"
    dst = out / "assets"
    dst.mkdir(parents=True, exist_ok=True)
    for name in ("style.css", "logo.png", "discuss.js"):
        s = src / name
        if not s.exists():
            raise SystemExit(f"missing asset: {s}")
        (dst / name).write_bytes(s.read_bytes())


def main() -> int:
    ap = argparse.ArgumentParser(description="Build the camareLOL site")
    ap.add_argument("--out", default=".")
    ap.add_argument(
        "--docs",
        default=str(Path(__file__).resolve().parent.parent / "docs"),
        help="Read documentation from this directory instead of the GitHub API, "
             "so a local build reflects the working tree rather than the last "
             "commit. CI has no docs/ and reads the API.",
    )
    args = ap.parse_args()

    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    copy_assets(out)
    rels = releases()
    if not rels:
        # Fail rather than publish. An empty news page and an empty docs section
        # look like a working site to a reader, and the build still reports
        # success — the front page quietly stops telling the truth. GitHub rate
        # limits unauthenticated calls, so this is reachable in normal use.
        log("could not read the release list; refusing to publish an empty site")
        return 1
    newest = rels[0]["tag_name"]
    log(f"{len(rels)} published releases; newest {newest}")
    docs = doc_list()

    (out / "index.html").write_text(page("Trang chủ", f"""  <section class="hero">
    <img src="/assets/logo.png" alt="camareLOL" width="120" height="118">
    <div>
      <h1>Trình cài đặt Arch Linux<br><span class="sub2">nói rõ trước khi xoá gì</span></h1>
      <p class="lede">camareLOL hỏi, cho xem kế hoạch, và chỉ ghi vào đĩa khi bạn gõ
      xác nhận. Bản cập nhật được ký số và tự kiểm tra, kể cả khi tải qua mirror.</p>
      <p class="cta">
        <a class="btn" href="https://github.com/{OWNER}/{REPO}/releases/latest">Tải {newest}</a>
        <a class="btn ghost" href="/docs.html">Đọc tài liệu</a>
      </p>
    </div>
  </section>

  <section>
    <h2>Bắt đầu nhanh</h2>
    <pre><code>curl -sSL -o /root/camarelol https://github.com/{OWNER}/{REPO}/releases/latest/download/camarelol
chmod +x /root/camarelol
/root/camarelol --self-test      # kiểm tra không phá huỷ
/root/camarelol --allow-destructive --execute</code></pre>
    <p class="note">Chạy từ ISO Arch sống. Xoá ổ đĩa không phải thứ để thử trên máy thật.</p>
  </section>


  <section>
    <h2>Tài liệu</h2>
    <ul class="docs">
{docs}
    </ul>
    <p class="note">Tài liệu được sinh tự động từ kho mã nguồn, nên không bao giờ lệch với
    mã. Mỗi trang có bản tiếng Việt, tiếng Anh và tiếng Trung.</p>
  </section>""", "index.html"), encoding="utf-8")

    (out / "news.html").write_text(page("Cập nhật",
        '  <h1>Cập nhật</h1>\n  <ol class="news">\n' + render_news(rels) +
        '\n  </ol>\n  <p class="note">Mỗi bản có <code>latest.json</code> kèm chữ ký GPG. Client kiểm\n'
        '  chữ ký bằng khoá nhúng trong binary, nên mirror không thể đưa nhầm bản.</p>',
        "news.html"), encoding="utf-8")

    (out / "docs.html").write_text(page("Tài liệu",
        '  <h1>Tài liệu</h1>\n'
        '  <p>Chọn ngôn ngữ. Mỗi bản được sinh tự động từ kho camareLOL, nên không bao giờ\n'
        '     lệch với mã nguồn.</p>\n'
        '  <ul class="docs">\n' + docs + "\n  </ul>", "docs.html"), encoding="utf-8")

    (out / "discuss.html").write_text(page("Thảo luận", """  <h1>Thảo luận</h1>
  <p class="lede">Đăng nhập bằng Google và viết ngay trên trang này. Bài viết vẫn nằm
  trong GitHub Discussions của kho camareLOL, nên không có nơi nào lưu bàn luận
  ngoài kho mã nguồn công khai.</p>

  <div id="signin" class="panel">
    <p>Chưa đăng nhập.</p>
    <p class="cta">
      <a class="btn" href="/api/auth-start">Đăng nhập bằng Google</a>
      <a class="btn ghost" href="https://github.com/OWNER/REPO/discussions">Xem trên GitHub</a>
    </p>
  </div>

  <form id="composer" hidden>
    <h2>Viết bài mới</h2>
    <p class="note" id="as-who"></p>
    <label for="title">Tiêu đề</label>
    <input id="title" name="title" maxlength="240" required
           placeholder="Tóm tắt vấn đề trong một dòng">
    <label for="body">Nội dung</label>
    <textarea id="body" name="body" rows="8" required
              placeholder="Càng cụ thể càng dễ có người trả lời."></textarea>
    <p class="cta">
      <button type="submit" class="btn">Đăng bài</button>
      <a class="btn ghost" href="/api/auth-logout">Đăng xuất</a>
    </p>
    <p class="note">Bài đăng mang tên Google của bạn, do bot của kho đăng lên —
    nên trên GitHub nó không hiện avatar GitHub của bạn.</p>
  </form>

  <p id="status" class="status" role="status" aria-live="polite"></p>

  <h2>Bài mới nhất</h2>
  <ul class="news" id="list"><li class="note">đang tải…</li></ul>

  <h2>Trước khi đăng</h2>
  <ul>
    <li>Báo lỗi thì dùng Issues — có mẫu điền, dễ theo dõi hơn thảo luận.</li>
    <li>Bản cài lỗi: ghi kèm phiên bản (<code>camarelol --version</code>) và ảnh chụp.</li>
    <li>Không dán khoá, mật khẩu hay token vào bài đăng.</li>
  </ul>

<script src="/assets/discuss.js" defer></script>""".replace("OWNER", OWNER).replace("REPO", REPO),
        "discuss.html"), encoding="utf-8")

    try:
        count = write_doc_pages(out, Path(args.docs) if args.docs else None)
    except Unpublishable as exc:
        log(f"refusing to publish: {exc}")
        return 1
    log(f"wrote {count} doc pages")
    if not count:
        log("no documentation was rendered; refusing to publish a site with no docs")
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())