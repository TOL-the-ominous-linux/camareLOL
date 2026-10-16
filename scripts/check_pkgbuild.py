#!/usr/bin/env python3
"""PKGBUILD phải khớp với bản phát hành mới nhất.

    python scripts/check_pkgbuild.py

PKGBUILD đã cứ nói 0.1.3 trong khi bản phát hành đã ở 0.1.10 — và `source` trỏ
tới một tarball không tồn tại, nên `makepkg -si` hỏng ngay dòng đầu. Số phiên
bản không tự nó phải đúng; nó phải được kiểm.

Chạy không cần mạng: đọc thẻ git, không gọi GitHub.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def newest_tag() -> str | None:
    try:
        out = subprocess.run(
            ["git", "-c", "safe.directory=*", "describe", "--tags", "--abbrev=0"],
            cwd=ROOT, capture_output=True, text=True, timeout=30,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    return out.stdout.strip() or None


def gitrev(rev: str) -> str | None:
    try:
        out = subprocess.run(
            ["git", "-c", "safe.directory=*", "rev-parse", rev],
            cwd=ROOT, capture_output=True, text=True, timeout=30,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    return out.stdout.strip() or None


def field(text: str, name: str) -> str | None:
    m = re.search(rf"^{name}=(.*)$", text, re.M)
    return m.group(1).strip().strip("'\"") if m else None


def main() -> int:
    path = ROOT / "PKGBUILD"
    if not path.exists():
        print("  không có PKGBUILD", file=sys.stderr)
        return 1
    text = path.read_text(encoding="utf-8")
    problems: list[str] = []

    pkgver = field(text, "pkgver")
    tag = newest_tag()
    if tag and pkgver != tag.lstrip("v"):
        tc, hc = gitrev(f"{tag}^{{commit}}"), gitrev("HEAD")
        if tc and hc and tc == hc:
            # Thẻ mới nhất trỏ đúng vào commit đang được build — khoảnh khắc
            # giữa chừng phát hành: thẻ được ghim trước, còn PKGBUILD xác định
            # kèm sha256 của tarball chỉ sinh ra ở commit sync ngay sau. Lần
            # chạy 0.1.13 theo thẻ đã đỏ oan ở đây, trên chính cây đã xanh
            # trước đó. Báo cho không im lặng, nhưng không báo hỏng.
            print(
                f"  PKGBUILD: giữa chừng phát hành ({tag} trỏ vào commit đang "
                "build) — pkgver cập nhật ở commit sync theo sau"
            )
        else:
            problems.append(
                f"pkgver={pkgver} nhưng thẻ mới nhất là {tag}. "
                f"Bản phát hành sinh lại PKGBUILD, nhưng bản trong kho vẫn phải đúng."
            )

    source = field(text, "source") or ""
    if "http" not in source:
        problems.append(
            "source không phải URL. makepkg sẽ tìm tệp trong thư mục hiện tại "
            "và hỏng: " + source
        )

    sums = re.search(r"^sha256sums=\((.*)\)$", text, re.M)
    entry = sums.group(1).strip().strip("'\"").strip("()").strip("'\"") if sums else ""
    if entry in ("", "SKIP"):
        problems.append(
            "sha256sums là SKIP. Đây là dự án kiểm tra sha256 trước khi cài, "
            "nên để ngoại trừ nó ở đây là im lặng sai khi không."
        )

    for problem in problems:
        print(f"  PKGBUILD: {problem}")
    if not problems:
        print(f"  PKGBUILD khớp {tag if tag else 'kho chưa có thẻ nào'} và có sha256 thật")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())