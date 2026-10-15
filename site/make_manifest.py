#!/usr/bin/env python3
"""Sinh manifest.json cho một lần deploy lên Cloudflare Pages.

    python site/make_manifest.py <thư mục> > manifest.json

Cloudflare Pages nhận danh sách tệp theo manifest khi deploy bằng API. Không có
nó, việc deploy phải dựng một tệp HTML trỏ về từng tệp riêng lẻ — chậm, và
một tệp quên mất là một link chết.

Thuộc tính được đặt rõ ràng thay vì để Cloudflare đoán: một trang tĩnh có
`latest.json` phải được phục vụ đúng kiểu, và `.sig` là văn bản đã ký chứ không
phải tệp nhị phân. Host nào đoán sai vẫn trả 200, và updater chỉ hỏng lúc chạy.
"""

from __future__ import annotations

import hashlib
import json
import mimetypes
import sys
from pathlib import Path

#: Kiểu nội dung đặt tay cho những tệp mà đoán theo đuôi sẽ sai.
CONTENT_TYPES = {
    ".json": "application/json",
    ".sig": "application/pgp-signature",
    ".js": "text/javascript",
    ".mjs": "text/javascript",
    ".html": "text/html; charset=utf-8",
    ".css": "text/css; charset=utf-8",
    ".txt": "text/plain; charset=utf-8",
    ".png": "image/png",
    ".md": "text/markdown; charset=utf-8",
}


def content_type(path: Path) -> str:
    suffix = path.suffix.lower()
    if suffix in CONTENT_TYPES:
        return CONTENT_TYPES[suffix]
    guessed, _ = mimetypes.guess_type(str(path))
    return guessed or "application/octet-stream"


def main() -> int:
    root = Path(sys.argv[1] if len(sys.argv) > 1 else ".")
    files: list[dict] = []

    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        rel = path.relative_to(root).as_posix()
        # Những thư mục này là đầu vào của build, không phải thứ để phục vụ.
        if rel.startswith((".git/", "site/", "keys/", "node_modules/")) or rel == ".gitignore":
            continue
        files.append({
            "path": rel,
            "content_type": content_type(path),
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "size": path.stat().st_size,
        })

    json.dump({"files": files}, sys.stdout, indent=2)
    print()
    return 0


if __name__ == "__main__":
    sys.exit(main())