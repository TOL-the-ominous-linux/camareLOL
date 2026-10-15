#!/usr/bin/env python3
"""Dịch phần comment sau `#` trong khối console, giữ nguyên phần lệnh.

    python site/translate_comments.py

Lệnh phải giống hệt — đó là thứ máy chạy. Nhưng comment sau `#` là văn xuôi
dành cho người đọc, và để nó tiếng Anh trong hướng dẫn tiếng Việt là hỏng một
nửa trang.

Bản dịch viết bằng tay ở đây, từng dòng một: cơ chế tự động không đáng tin
với văn bản cảnh báo, và cảnh báo sai thì tệ hơn không có.
"""

from __future__ import annotations

import pathlib
import re

DOCS = pathlib.Path(__file__).resolve().parent.parent / "docs"

#: file → {comment tiếng Anh: bản dịch}
VI = {
    "vm-testing.md": {
        "VM: Arch ISO + two empty disks": "VM: Arch ISO + hai đĩa rỗng",
        "headless; downloads the binary, runs self-test":
            "chạy không màn hình; tải binary, chạy self-test",
        "screenshot the result": "chụp lại kết quả",
        "JSON: model, serial, partitions, risk level":
            "JSON: model, số sê-ri, phân vùng, mức rủi ro",
        "non-destructive checks": "các bước kiểm không phá huỷ",
        "open the TUI; typing nothing writes nothing":
            "mở TUI; không gõ gì thì không ghi gì",
        "VM + empty 20 GiB disk + FAT disk holding the binary":
            "VM + đĩa rỗng 20 GiB + đĩa FAT chứa binary",
        "boot, type the install commands automatically":
            "boot, tự gõ các lệnh cài đặt",
        "delete the VM": "xoá VM",
        "NOT headless — you want to see it":
            "KHÔNG headless — bạn muốn nhìn màn hình",
        "mount -o remount,rw /          # a live ISO usually mounts read-only":
            "mount -o remount,rw /          # ISO sống thường mount read-only",
        "camarelol --self-test": "camarelol --self-test",
        "sudo ./camarelol --allow-destructive    # add --execute to actually write":
            "sudo ./camarelol --allow-destructive    # thêm --execute để ghi thật",
    },
    "testing.md": {
        "Rust core + TUI": "Rust core + TUI",
        "compiles, runs CatalogTest, builds the jar":
            "biên dịch, chạy CatalogTest, đóng gói jar",
        "site renderer": "bộ dựng trang",
        "site build, offline": "bản dựng trang, không mạng",
        "PKGBUILD matches the newest tag":
            "PKGBUILD khớp thẻ mới nhất",
    },
    "index.md": {
        "non-destructive checks; safe on a real machine":
            "kiểm không phá huỷ; an toàn trên máy thật",
        "the TUI": "mở TUI",
        "reports, downloads nothing, exit code 10 if newer":
            "chỉ báo, không tải gì, mã thoát 10 nếu có bản mới",
        "download, verify signature and SHA-256, install":
            "tải, kiểm chữ ký và SHA-256, cài",
        "put the previous binary back": "đưa binary cũ trở lại",
    },
}

ZH = {
    "vm-testing.md": {
        "VM: Arch ISO + two empty disks": "虚拟机：Arch ISO + 两块空磁盘",
        "headless; downloads the binary, runs self-test":
            "无头模式；下载二进制文件并运行自检",
        "screenshot the result": "截图保存结果",
        "JSON: model, serial, partitions, risk level":
            "JSON：型号、序列号、分区、风险等级",
        "non-destructive checks": "非破坏性检查",
        "open the TUI; typing nothing writes nothing":
            "打开 TUI；不输入任何内容就不会写入",
        "VM + empty 20 GiB disk + FAT disk holding the binary":
            "虚拟机 + 20 GiB 空磁盘 + 存放二进制文件的 FAT 磁盘",
        "boot, type the install commands automatically":
            "启动，自动输入安装命令",
        "delete the VM": "删除虚拟机",
        "NOT headless — you want to see it":
            "不要使用无头模式——你需要看到画面",
        "mount -o remount,rw /          # a live ISO usually mounts read-only":
            "mount -o remount,rw /          # 实时 ISO 通常以只读方式挂载",
        "sudo ./camarelol --allow-destructive    # add --execute to actually write":
            "sudo ./camarelol --allow-destructive    # 加上 --execute 才会真正写入",
    },
    "testing.md": {
        "Rust core + TUI": "Rust core + TUI",
        "compiles, runs CatalogTest, builds the jar":
            "编译、运行 CatalogTest 并打包 jar",
        "site renderer": "站点渲染器",
        "site build, offline": "站点构建，离线运行",
        "PKGBUILD matches the newest tag": "PKGBUILD 与最新标签一致",
    },
    "index.md": {
        "non-destructive checks; safe on a real machine":
            "非破坏性检查；在真实机器上运行也安全",
        "the TUI": "打开 TUI",
        "reports, downloads nothing, exit code 10 if newer":
            "只报告，不下载任何内容；有新版本时退出码为 10",
        "download, verify signature and SHA-256, install":
            "下载、验证签名与 SHA-256，然后安装",
        "put the previous binary back": "恢复上一个二进制文件",
    },
}


def translate(path: pathlib.Path, table: dict) -> int:
    text = path.read_text(encoding="utf-8")
    changed = 0
    for src, dst in table.items():
        needle = f"# {src}"
        if needle not in text:
            continue
        # Chỉ thay phần sau '#', giữ nguyên phần lệnh phía trước.
        pattern = re.compile(r"^(\s*\$ .*?)" + re.escape(needle) + r"\s*$", re.M)
        text, n = pattern.subn(lambda m: f"{m.group(1)}# {dst}", text)
        changed += n
    if changed:
        path.write_text(text, encoding="utf-8")
    return changed


def main() -> int:
    total = 0
    for lang, tables in (("vi", VI), ("zh", ZH)):
        for name, table in tables.items():
            p = DOCS / lang / name
            if not p.exists():
                print(f"  thiếu {p}")
                continue
            n = translate(p, table)
            print(f"  {lang}/{name}: {n} dòng")
            total += n
    print(f"  tổng {total} dòng comment đã dịch")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())