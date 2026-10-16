#!/usr/bin/env python3
"""Kiểm tra bảng Documentation trong trang chủ của mỗi ngôn ngữ.

    python scripts/check_lang_table.py

Bảng đó có ba cột ngôn ngữ và tồn tại để người đọc đang ở trang tiếng Việt bấm
"English" và thật sự sang trang tiếng Anh.

Cả ba cột đều là **đường dẫn tương đối tới file Markdown trong kho**: cột của
ngôn ngữ đang đọc là `ten-trang.md`, hai cột kia là `../<lang>/ten-trang.md`.
Không còn trang web nào, nên không có URL nào ở đây — và không cần, vì GitHub
hiển thị Markdown ngay trong kho.

Khi một cột khác bị viết thành đường dẫn tương đối **trong cùng thư mục** —
tức `architecture.md` thay vì `../en/architecture.md` — người đọc bấm "English"
và vẫn ở lại trang tiếng Việt. Không hỏng, không báo lỗi, chỉ là không làm gì cả:
loại lỗi tệ nhất khi bạn đang cố đọc thứ bằng ngôn ngữ khác.

Cả hai bản tiếng Việt và tiếng Trung đã mắc lỗi này.
"""

from __future__ import annotations

import pathlib
import re
import sys

DOCS = pathlib.Path(__file__).resolve().parent.parent / "docs"
LANGS = ("en", "vi", "zh")

ROW = re.compile(r"^\|\s*(?P<topic>[^|]+?)\s*\|(?P<cells>.+)\|\s*$")
LINK = re.compile(r"\[([^\]]*)\]\(([^)]+)\)")


def main() -> int:
    problems: list[str] = []
    checked = 0

    for lang in LANGS:
        path = DOCS / lang / "index.md"
        if not path.exists():
            problems.append(f"{lang}: không có {path}")
            continue

        in_table = False
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            if not line.startswith("|"):
                in_table = False
                continue

            cells = [c.strip() for c in line.strip().strip("|").split("|")]
            # Dòng kẻ: `|---|---|`.
            if set("".join(cells)) <= set("-: "):
                continue

            # `主題` là phồn thể, `主题` là giản thể — bảng tiếng Trung dùng
            # giản thể. Bỏ sót nó thì bảng cần kiểm không bao giờ được đọc.
            if cells[0] in ("Topic", "Chủ đề", "主題", "主题", "Subject"):
                in_table = True
                if len(cells) != 4:
                    problems.append(f"{lang}:{number} bảng có {len(cells)} cột, cần 4")
                continue
            if not in_table:
                continue

            for index, cell in enumerate(cells[1:4], start=1):
                target = LANGS[index - 1]
                m = LINK.search(cell)
                if not m:
                    problems.append(f"{lang}:{number} cột {target} không phải liên kết")
                    continue
                href = m.group(2)
                checked += 1

                if target == lang:
                    # Đường dẫn tương đối, không có `../`, phải tồn tại.
                    if href.startswith("../"):
                        problems.append(
                            f"{lang}:{number} cột {target} nên là đường dẫn "
                            f"trong thư mục, đang đi lên: {href}")
                    if not (path.parent / href).exists():
                        problems.append(f"{lang}:{number} liên kết chết: {href}")
                else:
                    # Phải đi sang thư mục ngôn ngữ khác, và phải tồn tại.
                    want = f"../{target}/"
                    if not href.startswith(want):
                        problems.append(
                            f"{lang}:{number} cột {target} phải bắt đầu bằng "
                            f"`{want}`, đang là `{href}` — bấm vào sẽ ở lại "
                            f"trang {lang}")
                    elif not (path.parent / href).exists():
                        problems.append(f"{lang}:{number} liên kết chết: {href}")

    if not checked:
        problems.append("không tìm thấy dòng nào trong bảng — bảng đã biến mất?")

    if problems:
        print(f"  {len(problems)} vấn đề trong bảng ngôn ngữ:")
        for p in problems[:20]:
            print(f"    {p}")
        return 1
    print(f"  bảng Documentation trỏ đúng ở cả ba ngôn ngữ ({checked} liên kết)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
