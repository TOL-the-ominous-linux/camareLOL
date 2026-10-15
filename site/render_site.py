#!/usr/bin/env python3
"""Build the camareLOL site: docs, release news, mirror status.

    python tools/render_site.py --out .

Everything is generated from the camareLOL repository, so the site cannot quietly
disagree with the code. Markdown is rendered by GitHub's own GFM renderer rather
than a local parser: no dependency to keep current, and the result is exactly
what people already read on GitHub.

This writes into the repository root because that is what Netlify publishes.
It never touches latest.json or latest.json.sig — those two files are the
update mirror, and a site build that clobbered them would break every client
that uses the default update URL.
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
FALLBACK_VERSION = "0.1.9"

#: Never written by the site build. These are the mirror.
MIRROR_FILES = ("latest.json", "latest.json.sig")


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


def render_markdown(text: str) -> str:
    """Render locally rather than through the GitHub API.

    The API route is gone — /repos/{owner}/{repo}/markdown now answers 404 even
    with a valid token — and its output carries GitHub's class names, which do
    not match this site's stylesheet, so the tables would arrive unstyled.
    """
    return markdown.render(text)


NAV = [
    ("index.html", "Trang chủ"),
    ("news.html", "Cập nhật"),
    ("status.html", "Trạng thái"),
    ("docs.html", "Tài liệu"),
    ("discuss.html", "Thảo luận"),
]


def page(title: str, body: str, active: str) -> str:
    items = "\n".join(
        '      <a href="/{href}"{on}>{label}</a>'.format(
            href=href, label=label, on=' class="on"' if href == active else ""
        )
        for href, label in NAV
    )
    return f"""<!doctype html>
<html lang="vi">
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
{items}
    </nav>
  </div>
</header>
<main class="wrap">
{body}
</main>
<footer class="wrap">
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
    return [r for r in rels if isinstance(r, dict) and not r.get("draft")]


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


def render_status(path: Path) -> str:
    if not path.exists():
        return ('    <p class="warn">Chưa có dữ liệu trạng thái. Chạy '
                '<code>python tools/check_mirrors.py --json</code> để tạo.</p>')
    data = json.loads(path.read_text())
    rows = []
    for m in data.get("mirrors", []):
        ok = m.get("ok")
        mark = '<span class="tag ok">đang sống</span>' if ok else '<span class="tag bad">hỏng</span>'
        detail = f"v{m['version']}" if ok and m.get("version") else html.escape(m.get("detail", ""))
        rows.append(f'    <tr><td>{mark}</td><td>{html.escape(m["url"])}</td><td>{detail}</td></tr>')
    stamp = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
    return (
        '  <table>\n'
        '    <thead><tr><th>Trạng thái</th><th>Địa chỉ</th><th>Chi tiết</th></tr></thead>\n'
        "    <tbody>\n" + "\n".join(rows) + "\n    </tbody>\n  </table>\n"
        f'  <p class="note">Kiểm tra lúc {stamp}. Một mirror hỏng không phải là hỏi: client thử '
        "lần lượt các địa chỉ còn lại, và bản nào trả lời được đều phải có chữ ký đúng.</p>"
    )


def doc_list() -> str:
    items = ['    <li><a href="/docs/README.html">README.md</a></li>']
    try:
        listing = get_json(f"{API}/contents/docs?ref={BRANCH}")
    except (urllib.error.URLError, OSError, json.JSONDecodeError):
        return items[0] + '\n    <li class="note">không đọc được danh sách tài liệu</li>'
    for f in sorted((x for x in listing if x["name"].endswith(".md")), key=lambda x: x["name"]):
        items.append(f'    <li><a href="/docs/{f["name"][:-3]}.html">{html.escape(f["name"])}</a></li>')
    return "\n".join(items)


def write_doc_pages(out: Path) -> int:
    docs = out / "docs"
    docs.mkdir(parents=True, exist_ok=True)
    targets = ["README.md"]
    try:
        listing = get_json(f"{API}/contents/docs?ref={BRANCH}")
        targets += [f"docs/{f['name']}" for f in sorted(listing, key=lambda x: x["name"])
                    if f["name"].endswith(".md")]
    except (urllib.error.URLError, OSError, json.JSONDecodeError):
        pass

    count = 0
    for path in targets:
        text = fetch_source(path)
        if text is None:
            log(f"skipped {path}: could not read it")
            continue
        slug = Path(path).stem
        title = slug.replace("-", " ").replace("_", " ").capitalize()
        rendered = render_markdown(text)
        (docs / f"{slug}.html").write_text(
            page(title, f"  <article>{rendered}</article>", ""), encoding="utf-8"
        )
        count += 1
    return count


def main() -> int:
    ap = argparse.ArgumentParser(description="Build the camareLOL site")
    ap.add_argument("--out", default=".")
    ap.add_argument("--status", default="mirror-status.json")
    args = ap.parse_args()

    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    rels = releases()
    newest = rels[0]["tag_name"] if rels else FALLBACK_VERSION
    log(f"{len(rels)} published releases; newest {newest}")
    status = render_status(Path(args.status))
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
    <h2>Tình trạng mirror</h2>
{status}
  </section>

  <section>
    <h2>Tài liệu</h2>
    <ul class="docs">
{docs}
    </ul>
  </section>""", "index.html"), encoding="utf-8")

    (out / "news.html").write_text(page("Cập nhật",
        '  <h1>Cập nhật</h1>\n  <ol class="news">\n' + render_news(rels) +
        '\n  </ol>\n  <p class="note">Mỗi bản có <code>latest.json</code> kèm chữ ký GPG. Client kiểm\n'
        '  chữ ký bằng khoá nhúng trong binary, nên mirror không thể đưa nhầm bản.</p>',
        "news.html"), encoding="utf-8")

    (out / "status.html").write_text(page("Trạng thái", f"""  <h1>Trạng thái</h1>
{status}
  <h2>Đường dẫn cập nhật</h2>
  <table>
    <thead><tr><th>Dùng để</th><th>Địa chỉ</th></tr></thead>
    <tbody>
      <tr><td>Mặc định</td><td><code>https://camarelol.netlify.app/latest.json</code></td></tr>
      <tr><td>Dự phòng</td><td><code>https://github.com/{OWNER}/{REPO}/releases/latest/download/latest.json</code></td></tr>
    </tbody>
  </table>
  <p class="note">Client thử lần lượt. Xem <a href="/docs/UPDATES.html">UPDATES.md</a>.</p>""",
        "status.html"), encoding="utf-8")

    (out / "docs.html").write_text(page("Tài liệu",
        '  <h1>Tài liệu</h1>\n  <p>Sinh tự động từ kho camareLOL, nên không bao giờ lệch với mã nguồn.</p>\n'
        '  <ul class="docs">\n' + docs + "\n  </ul>", "docs.html"), encoding="utf-8")

    (out / "discuss.html").write_text(page("Thảo luận", f"""  <h1>Thảo luận</h1>
  <p class="lede">Bàn luận nằm trong GitHub Discussions của kho camareLOL — không có máy chủ
  riêng, không có cơ sở dữ liệu riêng, và mọi bài đăng đều thuộc về kho mã nguồn công khai.</p>
  <p class="cta">
    <a class="btn" href="https://github.com/{OWNER}/{REPO}/discussions">Mở Discussions</a>
    <a class="btn ghost" href="https://github.com/{OWNER}/{REPO}/issues/new">Báo lỗi</a>
  </p>
  <h2>Trước khi đăng</h2>
  <ul>
    <li>Báo lỗi thì dùng Issues, có mẫu điền, dễ theo dõi hơn.</li>
    <li>Bản cài lỗi: ghi kèm phiên bản (<code>camarelol --version</code>) và ảnh chụp.</li>
    <li>Không dán khoá, mật khẩu hay token vào bài đăng.</li>
  </ul>""", "discuss.html"), encoding="utf-8")

    count = write_doc_pages(out)
    log(f"wrote {count} doc pages")

    missing = [f for f in MIRROR_FILES if not (out / f).exists()]
    if missing:
        log(f"WARNING: mirror files missing: {missing} — the site is served from this "
            f"directory, and a client using the default update URL needs them")
    return 0


if __name__ == "__main__":
    sys.exit(main())