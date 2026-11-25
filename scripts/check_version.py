#!/usr/bin/env python3
"""Một nguồn sự thật cho số phiên bản.

    python scripts/check_version.py

Cargo workspace, npm package và tệp ghi chú phát hành phải cùng một số.
PKGBUILD được `check_pkgbuild.py` đối chiếu với thẻ git; ở đây chỉ kiểm
những nơi không có thẻ để dựa vào. Chạy không cần mạng.

Lịch sử: PKGBUILD từng ghi 0.1.3 khi bản phát hành đã ở 0.1.10. Số phiên bản
không tự đúng; nó phải được kiểm.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def cargo_version() -> str | None:
    text = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    # Chỉ đọc version trong [workspace.package], không đọc version của crate.
    match = re.search(r"^\[workspace\.package\](.*?)(?=^\[|\Z)", text, re.M | re.S)
    if not match:
        return None
    version = re.search(r'^version\s*=\s*"([^"]+)"', match.group(1), re.M)
    return version.group(1) if version else None


def npm_version() -> str | None:
    try:
        data = json.loads((ROOT / "npm" / "package.json").read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    value = data.get("version")
    return value if isinstance(value, str) else None


def main() -> int:
    cargo = cargo_version()
    npm = npm_version()
    problems: list[str] = []

    if not cargo:
        problems.append("không đọc được version trong [workspace.package] của Cargo.toml")
    if not npm:
        problems.append("không đọc được version trong npm/package.json")
    if cargo and npm and cargo != npm:
        problems.append(f"Cargo.toml={cargo} nhưng npm/package.json={npm}")

    if cargo and not (ROOT / "docs" / "releases" / f"{cargo}.md").exists():
        problems.append(f"thiếu docs/releases/{cargo}.md cho phiên bản {cargo}")

    for problem in problems:
        print(f"  version: {problem}")
    if not problems:
        print(f"  version khớp: {cargo} (Cargo, npm, docs/releases)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
