#!/usr/bin/env python3
"""Kiểm tra mọi liên kết nội bộ của trang đã xuất bản.

    python site/check_links.py https://camarelol.netlify.app/

Không đoán xem trang có hỏng không — nó hỏi từng đường dẫn. Bản sinh ra có
đường dẫn đúng hình thức nhưng trỏ tới một tệp không tồn tại, và chỉ một
liên kết hỏng trong trang tài liệu mới khiến người đọc tin rằng tài liệu đó
không có.
"""

from __future__ import annotations

import re
import sys
import urllib.error
import urllib.parse
import urllib.request
from collections import deque

TIMEOUT = 25
UA = {"User-Agent": "camareLOL-link-check/1.0"}

_LINK = re.compile(r"""(?:href|src)\s*=\s*["']([^"']+)["']""", re.I)


def fetch(url: str) -> tuple[int, str]:
    req = urllib.request.Request(url, headers=UA)
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT) as resp:
            body = resp.read(400_000)
            ctype = resp.headers.get("Content-Type", "")
            if "text/html" in ctype:
                body = body.decode("utf-8", "replace")
            return resp.status, body if isinstance(body, str) else ""
    except urllib.error.HTTPError as exc:
        return exc.code, ""
    except (urllib.error.URLError, OSError):
        return 0, ""


def check(base: str) -> int:
    base = base.rstrip("/")
    origin = urllib.parse.urlsplit(base)

    seen: set[str] = set()
    queue: deque[str] = deque(["/"])
    broken: list[tuple[str, str, int]] = []
    skipped_api: set[str] = set()
    pages = 0

    while queue:
        path = queue.popleft()
        if path in seen:
            continue
        seen.add(path)

        status, body = fetch(base + path)
        pages += 1
        if status != 200:
            broken.append((path, "trang", status))
            continue

        for raw in _LINK.findall(body):
            target = raw.strip()
            if not target or target.startswith(("mailto:", "data:", "javascript:", "#")):
                continue
            absolute = urllib.parse.urljoin(base + path, target)
            parts = urllib.parse.urlsplit(absolute)
            if parts.netloc and parts.netloc != origin.netloc:
                continue  # ra ngoài site, không thuộc phạm vi
            clean = parts.path + (f"?{parts.query}" if parts.query else "")
            if clean.startswith("/latest.json"):
                continue  # tệp chữ ký, đã kiểm riêng
            if clean.startswith("/api/"):
                # Served by Netlify functions, not by files. A static server
                # returns 404 for every one of them, so checking here reports
                # the local preview as broken rather than the site. Skip, and
                # say so, rather than teaching the checker to guess.
                skipped_api.add(clean)
                continue
            name = parts.path.rsplit("/", 1)[-1]
            looks_like_page = (
                clean.endswith((".html", "/"))
                # Đường dẫn không đuôi vẫn là trang: /news, /status, /docs. Nếu
                # chỉ nhận .html thì bộ dò dừng ở trang chủ và báo là không có
                # liên kết hỏng — tức là báo rằng trang chủ sạch trong khi
                # chưa từng nhìn vào trang nào khác.
                or "." not in name
            )
            if not looks_like_page:
                status_asset, _ = fetch(base + clean)
                if status_asset != 200:
                    broken.append((path, clean, status_asset))
                continue
            queue.append(clean)

    print(f"  đã kiểm {pages} trang")
    if skipped_api:
        print(f"  bỏ qua {len(skipped_api)} đường dẫn /api/* (do function phục vụ, "
              f"không phải tệp)")
    if not broken:
        print("  không có liên kết nội bộ hỏng")
        return 0

    print(f"  {len(broken)} liên kết hỏng:")
    for path, target, status in sorted(set(broken)):
        print(f"    {status or 'lỗi':>4}  {target}   (từ {path})")
    return 1


if __name__ == "__main__":
    target = sys.argv[1] if len(sys.argv) > 1 else "https://camarelol.netlify.app"
    sys.exit(check(target))