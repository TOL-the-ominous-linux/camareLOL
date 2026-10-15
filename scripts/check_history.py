#!/usr/bin/env python3
"""Kiểm tra bí mật có còn trong lịch sử git hay không.

    python scripts/check_history.py
    python scripts/check_history.py --expect-clean "GOCSPX-xxxxxxxx"

`check_secrets.py` nhìn cây làm việc. Tệp này nhìn tất cả: mỗi commit, mọi
phiên bản của mỗi tệp. Một bí mật đã bị xoá khỏi `main` vẫn nằm trong lịch sử,
và kẻ quét tài khoản công khai thấy nó ở đó.

Exit code khác 0 nghĩa là còn. Đó là cố ý: sau khi viết lại lịch sử, chạy lại
lệnh này là cách biết việc đó có thật sự xong không, thay vì tin rằng nó xong.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

PATTERNS: list[tuple[str, re.Pattern[bytes]]] = [
    ("GitHub token", re.compile(rb"\bgh[pousr]_[A-Za-z0-9]{36,}\b")),
    ("Google OAuth client secret", re.compile(rb"\bGOCSPX-[A-Za-z0-9_-]{20,}\b")),
    ("Google API key", re.compile(rb"\bAIza[0-9A-Za-z_-]{35}\b")),
    ("AWS access key id", re.compile(rb"\bAKIA[0-9A-Z]{16}\b")),
    ("private key block", re.compile(rb"-----BEGIN [A-Z ]*PRIVATE KEY-----")),
]


def commits() -> list[str]:
    out = subprocess.run(
        ["git", "-c", "safe.directory=*", "rev-list", "--all"],
        cwd=ROOT, capture_output=True, text=True, timeout=120)
    return out.stdout.split()


def search(pattern: re.Pattern[bytes]) -> list[str]:
    """Commit nào chứa mẫu này. Chậm, nhưng đây là kiểm tra chạy thủ công."""
    proc = subprocess.Popen(
        ["git", "-c", "safe.directory=*", "log", "--all", "-S", pattern.pattern.decode(),
         "--pickaxe-regex", "--pretty=format:%h %s"],
        cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    found, _ = proc.communicate(timeout=300)
    return [l for l in found.splitlines() if l.strip()]


def search_messages(pattern: re.Pattern[bytes]) -> list[str]:
    """Nội dung commit, không phải nội dung tệp.

    `git log -S` chỉ dò thay đổi trong tệp. Một token nằm trong thông điệp commit
    là công khai y hệt — GitHub hiện nó ngay bên cạnh mã — và bản đầu tiên của
    công cụ này báo "sạch" trong khi nó còn đó.
    """
    proc = subprocess.Popen(
        ["git", "-c", "safe.directory=*", "log", "--all", "--format=%H%x00%B%x00%x00"],
        cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    out, _ = proc.communicate(timeout=300)
    hits = []
    for block in out.split(b"\x00\x00"):
        if not block.strip():
            continue
        sha, _, message = block.partition(b"\x00")
        if pattern.search(message):
            first = message.decode("utf-8", "replace").strip().splitlines()
            hits.append(f"{sha.decode()[:7]} (nội dung commit) {first[0][:60] if first else ''}")
    return hits


def main() -> int:
    expect = None
    if "--expect-clean" in sys.argv:
        i = sys.argv.index("--expect-clean")
        expect = sys.argv[i + 1]

    if expect:
        pattern = re.compile(re.escape(expect))
        hits = search(pattern) + search_messages(pattern)
        if hits:
            print(f"  '{expect}' vẫn xuất hiện ở {len(hits)} chỗ:")
            for h in hits[:10]:
                print(f"    {h}")
            return 1
        print(f"  '{expect}' không còn trong lịch sử")
        return 0

    print(f"  quét {len(commits())} commit, cả nội dung tệp lẫn thông điệp commit")
    total = 0
    for name, pattern in PATTERNS:
        hits = search(pattern) + search_messages(pattern)
        if hits:
            total += len(hits)
            print(f"  {name}: {len(hits)} chỗ")
            for h in hits[:5]:
                print(f"    {h}")
        else:
            print(f"  {name}: sạch")

    if total:
        print("\n  Bí mật trong lịch sử không tự biến mất khi bạn xoá tệp ở main.")
        print("  Thu hồi nó trước — làm cho nó vô hiệu — rồi mới viết lại lịch sử.")
    return 1 if total else 0


if __name__ == "__main__":
    sys.exit(main())