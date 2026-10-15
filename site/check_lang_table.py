#!/usr/bin/env python3
"""Kiểm tra bảng Documentation trong trang chủ của mỗi ngôn ngữ.

    python site/check_lang_table.py

Bảng đó có ba cột ngôn ngữ. Cột của chính ngôn ngữ đang đọc được viết bằng
đường dẫn tương đối (`updates-signing.md`) — đúng. Hai cột còn lại **phải** là
URL tuyệt đối tới trang của ngôn ngữ đó.

Khi một cột khác bị viết thành đường dẫn tương đối, người đọc bấm "English" và
vẫn ở lại trang tiếng Việt. Không hỏng, không báo lỗi, chỉ là không làm gì cả —
loại lỗi tệ nhất khi bạn đang cố đọc thứ bằng ngôn ngữ khác.

Bản dịch tiếng Việt đã mắc lỗi này: cột English trỏ `updates-signing.md`.
"""

from __future__ import annotations

import pathlib
import re
import sys

DOCS = pathlib.Path(__file__).resolve().parent.parent / "docs"
SITE = "https://tamkogay9876-code.github.io/camareLOL-mirror"
LANGS = {"en": "English", "vi": "Tiếng Việt", "zh": "中文"}

ROW = re.compile(r"^\|\s*(?P<topic>[^|]+?)\s*\|(?P<cells>.+)\|\s*$")
LINK = re.compile(r"\[([^\]]*)\]\(([^)]+)\)")


def main() -> int:
    problems: list[str] = []
    for lang in sorted(LANGS):
        path = DOCS / lang / "index.md"
        if not path.exists():
            problems.append(f"{lang}: không có {path}")
            continue

        in_table = False
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if not line.startswith("|"):
                in_table = False
                continue
            # Cột đầu là tiêu đề cột, không phải dữ liệu.
            cells = [c.strip() for c in line.strip().strip("|").split("|")]
            if set("".join(cells)) <= set("-: "):
                continue
            if cells[0] in ("Topic", "Chủ đề", "主題", "Subject"):
                in_table = True
                if len(cells) != 4:
                    problems.append(f"{lang}:{number} bảng có {len(cells)} cột, cần 4")
                continue
            if not in_table:
                continue

            for index, cell in enumerate(cells[1:], start=1):
                if index > 3:
                    continue
                target_lang = sorted(LANGS)[index - 1]
                m = LINK.search(cell)
                if not m:
                    continue
                href = m.group(2)
                is_relative = not href.startswith(("http://", "https://"))

                if target_lang == lang:
                    if not is_relative:
                        problems.append(
                            f"{lang}:{number} cột {target_lang} nên là đường dẫn "
                            f"tương đối, đang là URL: {href}")
                elif is_relative:
                    problems.append(
                        f"{lang}:{number} cột {target_lang} là đường dẫn tương đối "
                        f"({href}) — bấm vào sẽ ở lại trang {lang}. "
                        f"Phải là {SITE}/{target_lang}/docs/...")
                elif f"/{target_lang}/docs/" not in href:
                    problems.append(
                        f"{lang}:{number} cột {target_lang} trỏ sang nơi khác: {href}")

    if problems:
        print(f"  {len(problems)} vấn đề trong bảng ngôn ngữ:")
        for p in problems[:20]:
            print(f"    {p}")
        return 1
    print("  bảng Documentation trỏ đúng ở cả ba ngôn ngữ")
    return 0


if __name__ == "__main__":
    sys.exit(main())