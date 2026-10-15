#!/usr/bin/env python3
"""Chặn bí mật trước khi nó được commit.

    python scripts/check_secrets.py

.gitignore ngăn được tai nạn: bạn tạo tệp cấu hình trong thư mục làm việc và nó
lọt vào commit. Nó không ngăn được việc bạn `git add -f`, hay dán bí mật thẳng
vào một tệp đã được theo dõi. Đây là lớp thứ hai, và nó chạy trong CI.

Quét hai thứ: tệp đang được theo dõi, và nội dung các thay đổi chưa commit.
Cả hai đều cần — một bí mật nằm trong lịch sử git thì `git rm` không còn giữ
được bí mật đó.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

#: (tên, mẫu). Mỗi mẫu càng hẹp càng tốt: quét rộng sẽ báo nhầm trên mã nguồn
#: bình thường, và một phép kiểm tra báo nhầm thì sẽ bị bỏ qua.
PATTERNS: list[tuple[str, re.Pattern[str]]] = [
    ("GitHub token", re.compile(rb"\bgh[pousr]_[A-Za-z0-9]{36,}\b")),
    ("Google OAuth client secret", re.compile(rb"\bGOCSPX-[A-Za-z0-9_-]{20,}\b")),
    ("Google API key", re.compile(rb"\bAIza[0-9A-Za-z_-]{35}\b")),
    ("AWS access key id", re.compile(rb"\bAKIA[0-9A-Z]{16}\b")),
    ("private key block", re.compile(rb"-----BEGIN [A-Z ]*PRIVATE KEY-----")),
    ("Slack token", re.compile(rb"\bxox[abprs]-[A-Za-z0-9-]{10,}\b")),
    ("generic assignment", re.compile(
        rb"(?i)\b(?:api[_-]?key|client[_-]?secret|auth[_-]?token|password)\s*[=:]\s*"
        rb"['\"][^'\"\s]{16,}['\"]")),
]

#: Nơi một mẫu trùng là bình thường.
ALLOWED = ("test_markdown.py", "test_render_site.py", "test_lol.py",
           "check_secrets.py", "translate_comments.py")

TEXT_SUFFIXES = {".rs", ".py", ".js", ".ts", ".html", ".css", ".md", ".yml",
                 ".yaml", ".sh", ".toml", ".json", ".java", ".txt", ""}


def tracked_files() -> list[str]:
    out = subprocess.run(["git", "-c", "safe.directory=*", "ls-files"],
                         cwd=ROOT, capture_output=True, text=True, timeout=60)
    return [p for p in out.stdout.splitlines() if p]


def untracked_files() -> list[str]:
    """Files that are not ignored and not tracked yet.

    The usual way a secret reaches a commit: `git add .` on a directory where
    somebody left a credential. It is in neither the tracked list nor the diff
    until the moment it is committed, so scanning only those two misses exactly
    the case that matters.
    """
    out = subprocess.run(
        ["git", "-c", "safe.directory=*", "ls-files", "--others", "--exclude-standard"],
        cwd=ROOT, capture_output=True, text=True, timeout=60)
    return [p for p in out.stdout.splitlines() if p]


def scan(label: str, blobs: dict[str, bytes]) -> list[str]:
    found = []
    for path, data in blobs.items():
        if any(a in path for a in ALLOWED):
            continue
        for name, pattern in PATTERNS:
            if pattern.search(data):
                found.append(f"{label}: {path} — có thể là {name}")
    return found


def main() -> int:
    problems: list[str] = []

    blobs: dict[str, bytes] = {}
    for rel in tracked_files():
        p = ROOT / rel
        if p.suffix.lower() not in TEXT_SUFFIXES or not p.is_file():
            continue
        try:
            blobs[rel] = p.read_bytes()
        except OSError:
            continue
    print(f"  quét {len(blobs)} tệp đang được theo dõi")
    problems += scan("đã theo dõi", blobs)

    new: dict[str, bytes] = {}
    for rel in untracked_files():
        p = ROOT / rel
        if p.suffix.lower() not in TEXT_SUFFIXES or not p.is_file():
            continue
        try:
            new[rel] = p.read_bytes()
        except OSError:
            continue
    print(f"  quét {len(new)} tệp mới chưa theo dõi")
    problems += scan("chưa theo dõi", new)

    # Thay đổi chưa commit, kể cả tệp chưa theo dõi.
    diff = subprocess.run(["git", "-c", "safe.directory=*", "diff", "HEAD"],
                          cwd=ROOT, capture_output=True, timeout=60)
    if diff.returncode == 0 and diff.stdout:
        # Chỉ dòng được thêm. Một dòng đang bị xoá là lần sửa, không phải
        # lần thêm — và quét cả nó làm cho "xoá một bí mật khỏi tệp" thành
        # việc phải làm qua hai lần commit trước khi sạch.
        added = b"\n".join(
            line for line in diff.stdout.split(b"\n")
            if line.startswith(b"+") and not line.startswith(b"+++")
        )
        print(f"  quét {len(added)} byte dòng được thêm")
        for index in (0, 1):
            if PATTERNS[index][1].search(added):
                problems.append(
                    f"dòng mới thêm chứa một mẫu bí mật ({PATTERNS[index][0]})")
                break

    if problems:
        print(f"\n  {len(problems)} vấn đề:")
        for p in problems:
            print(f"    {p}")
        print("\n  Nếu đây là mẫu giả trong test, thêm tệp vào ALLOWED trong "
              "scripts/check_secrets.py —\n  đừng nới mẫu.")
        return 1

    # Thư mục cách ly phải nằm ngoài kho. Nếu ai đó di chuyển nó vào trong, nó sẽ
    # bị .gitignore ở chỗ kia chặn — nhưng chỉ cho tới lúc ai đó chạy `git add -f`.
    # Kiểm tra ở đây thì thấy ngay, trước khi nó thành bí mật trong lịch sử.
    quarantine = Path.home() / "Tài liệu" / "cắt cy"
    try:
        quarantine.resolve().relative_to(ROOT.resolve())
    except ValueError:
        print(f"  thư mục cách ly nằm ngoài kho: {quarantine}")
    except OSError:
        pass  # không có trên máy này; không phải lỗi
    else:
        problems.append(f"thư mục cách ly {quarantine} nằm BÊN TRONG kho — "
                        f"bí mật sẽ bị commit")

    if problems:
        print(f"\n  {len(problems)} vấn đề:")
        for item in problems:
            print(f"    {item}")
        return 1

    print("  không thấy bí mật nào trong tệp đang được theo dõi hay thay đổi chưa commit")
    return 0


if __name__ == "__main__":
    sys.exit(main())
