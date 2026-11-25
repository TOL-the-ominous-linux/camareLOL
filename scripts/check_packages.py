#!/usr/bin/env python3
"""Mọi tên gói camareLOL có thể đưa cho pacman đều phải tồn tại.

    python scripts/check_packages.py            # Arch + AUR (cần mạng)
    python scripts/check_packages.py --offline  # chỉ đối chiếu ba catalog

Catalog Rust là nguồn sự thật. Gợi ý của Python và catalog Java được chép từ
đó, nên một cái tên có thể lệch ở ba nơi:

* `core/src/packages.rs`            — `official(...)` / `official_for(...)` / `aur(...)`
* `python/.../package_recommend.py` — mọi gói `recommend()` có thể trả về
* `java/.../PackageCatalog.java`    — `PackageEntry.of(...)` và hồ sơ desktop

Một cái tên sai đi qua toàn bộ unit test và hỏng trong VM tại `pacstrap`.
`greetd-tuigreeter` và `p7zip` đã làm đúng như vậy; `fcitx5-bamboo` là cái
tiếp theo, rồi `nvidia` khi driver 590 thay nó bằng `nvidia-open`. Script này
kiểm tên với kho Arch và AUR đang sống, và cũng không để ba catalog nói khác
nhau.

Tìm kiếm của Arch phân biệt một gói với một *nhóm* gói (`plasma`,
`xfce4-goodies`) và với một tên được gói khác *cung cấp* (`libva-mesa-driver`
do `mesa` cung cấp). Cả ba đều cài được, nên cả ba đều tính là tồn tại. AUR
không có nhóm hay provides: một tên hoặc có, hoặc không.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

OFFICIAL_BY_NAME = "https://archlinux.org/packages/search/json/?name={}"
OFFICIAL_QUERY = "https://archlinux.org/packages/search/json/?q={}"
OFFICIAL_GROUP = "https://archlinux.org/groups/x86_64/{}/"
AUR_INFO = "https://aur.archlinux.org/rpc/v5/info?{}"
USER_AGENT = (
    "camareLOL-package-check/1.0 "
    "(+https://github.com/TOL-the-ominous-linux/camareLOL)"
)


class NetworkError(RuntimeError):
    """Không tới được Arch; đây không phải là một gói bị thiếu."""


def fetch(url: str, timeout: float) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            return response.read()
    except (urllib.error.URLError, TimeoutError, OSError) as exc:
        raise NetworkError(f"{url}: {exc}") from exc


def fetch_json(url: str, timeout: float) -> dict:
    try:
        return json.loads(fetch(url, timeout))
    except json.JSONDecodeError as exc:
        raise NetworkError(f"{url}: JSON không hợp lệ: {exc}") from exc


def http_status(url: str, timeout: float) -> int:
    """HTTP status, with 404 kept as a real answer.

    A group name is checked on its own page rather than by scraping the group
    index: the index's HTML has changed before, and a parser that silently
    returns an empty set turns every group into a false "missing" package.
    """
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    try:
        with urllib.request.urlopen(request, timeout=timeout) as response:
            return response.status
    except urllib.error.HTTPError as exc:
        if exc.code == 404:
            return 404
        raise NetworkError(f"{url}: HTTP {exc.code}") from exc
    except (urllib.error.URLError, TimeoutError, OSError) as exc:
        raise NetworkError(f"{url}: {exc}") from exc


def rust_catalog() -> tuple[set[str], set[str]]:
    text = (ROOT / "core" / "src" / "packages.rs").read_text(encoding="utf-8")
    official = set(re.findall(r"\bofficial(?:_for)?\(\s*\"([^\"]+)\"", text))
    aur = set(re.findall(r"\baur\(\s*\"([^\"]+)\"", text))
    if not official or not aur:
        raise SystemExit("không đọc được catalog Rust trong core/src/packages.rs")
    return official, aur


def python_catalog() -> tuple[set[str], set[str]]:
    sys.path.insert(0, str(ROOT / "python"))
    try:
        from camarelol_tools import package_recommend as recommend
    finally:
        sys.path.pop(0)

    everything: set[str] = set()
    aur: set[str] = set()
    # Gọi chính hàm `recommend()`, không chỉ đọc bảng: nó tự thêm
    # `xdg-desktop-portal`, font Noto và `ibus` trong các nhánh của mình.
    for desktop in recommend.DESKTOP_PACKAGES:
        for server in ("auto", "wayland", "x11"):
            for audio in recommend.AUDIO_PACKAGES:
                for language in ("en", "vi"):
                    for use_aur in (False, True):
                        rec = recommend.recommend(
                            desktop=desktop,
                            display_server=server,
                            audio=audio,
                            language=language,
                            use_aur=use_aur,
                            extra_groups=list(recommend.UTILITY_GROUPS),
                        )
                        everything.update(rec.packages)
                        if use_aur:
                            aur.update(rec.aur)
    return everything - aur, aur


def java_catalog() -> tuple[set[str], set[str]]:
    path = (
        ROOT
        / "java"
        / "src"
        / "main"
        / "java"
        / "com"
        / "camarelol"
        / "catalog"
        / "PackageCatalog.java"
    )
    text = path.read_text(encoding="utf-8")

    official: set[str] = set()
    aur: set[str] = set()
    for name, source in re.findall(
        r"PackageEntry\.of\(\s*\"([^\"]+)\"\s*,\s*\"[^\"]*\"\s*,\s*Source\.(OFFICIAL|AUR)\)",
        text,
    ):
        (aur if source == "AUR" else official).add(name)

    # Chỉ String[] đầu tiên của DesktopEntry là danh sách gói; các mảng sau
    # là display manager, display server và service.
    for block in re.findall(
        r"new DesktopEntry\(\s*\"[^\"]+\"\s*,\s*new String\[\]\{([^}]*)\}", text
    ):
        official.update(re.findall(r"\"([^\"]+)\"", block))

    if not official or not aur:
        raise SystemExit(f"không đọc được catalog Java trong {path}")
    return official, aur


def cross_source_drift(
    rust_official: set[str],
    rust_aur: set[str],
    others: dict[str, tuple[set[str], set[str]]],
) -> list[str]:
    problems: list[str] = []
    for source, (official, aur) in others.items():
        for name in sorted(official - rust_official):
            problems.append(
                f"{source}: {name} là official, nhưng catalog Rust không liệt kê"
            )
        for name in sorted(aur - rust_aur):
            problems.append(
                f"{source}: {name} là AUR, nhưng catalog Rust không liệt kê"
            )
        for name in sorted(official & rust_aur):
            problems.append(
                f"{source}: {name} là official, nhưng catalog Rust ghi là AUR"
            )
        for name in sorted(aur & rust_official):
            problems.append(
                f"{source}: {name} là AUR, nhưng catalog Rust ghi là official"
            )
    return problems


def official_status(
    name: str, timeout: float, cache: dict[str, tuple[str, str] | None]
) -> tuple[str, str] | None:
    if name in cache:
        return cache[name]

    data = fetch_json(OFFICIAL_BY_NAME.format(urllib.parse.quote(name)), timeout)
    exact = [r for r in data.get("results", []) if r.get("pkgname") == name]
    if exact:
        cache[name] = ("package", exact[0].get("repo", "?"))
        return cache[name]

    # Một nhóm gói (`plasma`, `xfce4-goodies`) cài được như một tên, nhưng
    # không xuất hiện trong kết quả tìm gói. Hỏi thẳng trang của nhóm.
    time.sleep(0.05)
    group_url = OFFICIAL_GROUP.format(urllib.parse.quote(name))
    if http_status(group_url, timeout) == 200:
        cache[name] = ("group", "group")
        return cache[name]

    # Một tên ảo vẫn cài được nếu một gói official cung cấp nó.
    time.sleep(0.05)
    data = fetch_json(OFFICIAL_QUERY.format(urllib.parse.quote(name)), timeout)
    for result in data.get("results", []):
        for provided in result.get("provides") or []:
            if provided == name or provided.startswith(name + "="):
                cache[name] = ("provides", result.get("pkgname", "?"))
                return cache[name]

    cache[name] = None
    return None


def aur_found(names: set[str], timeout: float) -> set[str]:
    if not names:
        return set()
    params = "&".join(
        "arg%5B%5D=" + urllib.parse.quote(name) for name in sorted(names)
    )
    data = fetch_json(AUR_INFO.format(params), timeout)
    return {result.get("Name") for result in data.get("results", [])}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="kiểm tra mọi tên gói của camareLOL có tồn tại"
    )
    parser.add_argument(
        "--offline",
        action="store_true",
        help="chỉ kiểm tra catalog Rust, Python và Java có khớp nhau",
    )
    parser.add_argument(
        "--timeout", type=float, default=30.0, help="timeout mỗi request (giây)"
    )
    args = parser.parse_args(argv)

    rust_official, rust_aur = rust_catalog()
    python_official, python_aur = python_catalog()
    java_official, java_aur = java_catalog()

    problems = cross_source_drift(
        rust_official,
        rust_aur,
        {
            "Python": (python_official, python_aur),
            "Java": (java_official, java_aur),
        },
    )
    for problem in problems:
        print(f"  LỆCH    {problem}")

    if args.offline:
        print(
            f"offline: {len(rust_official)} official + {len(rust_aur)} AUR trong "
            f"catalog Rust; {len(problems)} chỗ ba catalog không khớp"
        )
        return 1 if problems else 0

    official = rust_official | python_official | java_official
    aur = rust_aur | python_aur | java_aur
    missing: list[str] = []

    try:
        cache: dict[str, tuple[str, str] | None] = {}
        for name in sorted(official):
            status = official_status(name, args.timeout, cache)
            if status is None:
                missing.append(name)
                print(f"  THIẾU   official  {name}")
            else:
                kind, detail = status
                print(f"  OK      official  {name:<28} {kind} {detail}")

        found = aur_found(aur, args.timeout)
        for name in sorted(aur):
            if name in found:
                print(f"  OK      aur       {name}")
            else:
                missing.append(name)
                print(f"  THIẾU   aur       {name}")
    except NetworkError as exc:
        print(f"  lỗi mạng: {exc}", file=sys.stderr)
        return 2

    if problems or missing:
        print(
            f"\n{len(problems)} chỗ ba catalog không khớp, "
            f"{len(missing)} tên gói không tồn tại"
        )
        return 1

    print(f"\ntất cả {len(official)} tên official và {len(aur)} tên AUR đều tồn tại")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
