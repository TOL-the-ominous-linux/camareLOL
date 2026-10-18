#!/usr/bin/env python3
"""Kết luận về kết quả kiểm tra trong VirtualBox.

    python scripts/vm-check-results.py --results /tmp/opencode/vm-accept --expect-version 0.1.11

Đọc các tệp khách gửi về và kiểm từng cái. Trả 0 khi tất cả đạt, khác 0 khi có
bất kỳ cái nào hỏng.

Không có bước nào ở đây là "chụp ảnh rồi coi như xong". Mỗi kiểm tra đều đọc một
chuỗi cụ thể và nói rõ nó đang tìm cái gì.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

#: (tên bước, mô tả, kiểm tra) — kiểm tra nhận toàn văn kết quả, trả về
#: (đạt, lý do nếu không).
CHECKS: list[tuple[str, str, object]] = []


def check(step: str):
    def wrap(fn):
        CHECKS.append((step, fn.__doc__ or "", fn))
        return fn
    return wrap


def text(results: pathlib.Path, step: str) -> str:
    path = results / f"step_{step}.txt"
    if not path.exists():
        return ""
    return path.read_text(encoding="utf-8", errors="replace")


def rc_of(body: str) -> int | None:
    """Mã thoát mà khách ghi lại.

    Khách luôn viết `rc=<n>` trước nội dung, nên chỉ cần dò đầu tiên.
    """
    m = re.search(r"^rc=(\d+)", body, re.M)
    return int(m.group(1)) if m else None


@check("download")
def _(body: str, expect: str) -> tuple[bool, str]:
    """Binary tải từ máy chủ phải chạy và tự nhận đúng phiên bản."""
    if not body:
        return False, "không có kết quả — khách chưa gửi, hoặc treo trước bước này"
    if "downloaded" not in body:
        return False, f"tải binary thất bại: {body.strip()[:120]}"
    m = re.search(r"camareLOL (\S+)", body)
    if not m:
        return False, f"--version không in ra gì nhận dạng được: {body.strip()[:120]}"
    if m.group(1) != expect:
        return False, f"binary tự báo {m.group(1)}, cần {expect}"
    if rc_of(body) not in (0, None):
        return False, f"--version trả mã {rc_of(body)}"
    if f"tag v{expect}" not in body:
        return False, f"binary không mang thẻ v{expect}: {body.strip()[:120]}"
    return True, f"{expect}, thẻ v{expect} khớp"


@check("scan")
def _(body: str, expect: str) -> tuple[bool, str]:
    """--scan phải đọc được đĩa thật trong VM."""
    rc = rc_of(body)
    if rc is None:
        return False, f"không có mã thoát: {body.strip()[:120]}"
    if rc != 0:
        return False, f"--scan trả mã {rc}: {body.strip()[:200]}"
    m = re.search(r"bytes=(\d+)", body)
    if not m or int(m.group(1)) < 10:
        return False, f"survey rỗng ({m.group(1) if m else '?'} byte)"
    return True, f"survey {m.group(1)} byte"


@check("selftest")
def _(body: str, expect: str) -> tuple[bool, str]:
    """--self-test phải tự báo đạt trên phần cứng thật."""
    if not body:
        return False, "không có kết quả"
    rc = rc_of(body)
    if rc is not None and rc != 0:
        return False, f"--self-test trả mã {rc}: {body.strip()[:300]}"
    if "selftest result: PASS" not in body:
        return False, f"không thấy 'selftest result: PASS': {body.strip()[:300]}"
    return True, "PASS"


@check("help")
def _(body: str, expect: str) -> tuple[bool, str]:
    """--help không được còn bất kỳ cờ cập nhật nào."""
    if not body:
        return False, "không có kết quả"
    rc = rc_of(body)
    if rc is not None and rc != 0:
        return False, f"--help trả mã {rc}"
    m = re.search(r"^(\d+)$", body, re.M)
    count = int(m.group(1)) if m else None
    if count is None:
        return False, "không đếm được số cờ cập nhật trong --help"
    if count != 0:
        return False, f"--help vẫn còn {count} cờ cập nhật"
    if "does not update itself" not in body:
        return False, "--help không nói rõ là không tự cập nhật"
    return True, "không cờ cập nhật, và nói lý do"


@check("install_profile")
def _(body: str, expect: str) -> tuple[bool, str]:
    """Profile cài đặt phải sinh được từ khảo sát đĩa thật."""
    if not body:
        return False, "không có kết quả — vm-helper.py có thể đã chết"
    rc = re.search(r"profile_rc=(\d+)", body)
    if rc is None:
        return False, f"không có profile_rc: {body.strip()[:200]}"
    if rc.group(1) != "0":
        return False, f"vm-helper.py trả mã {rc.group(1)}: {body.strip()[:200]}"
    if not re.search(r"targeting /dev/", body):
        return False, f"profile không trỏ vào ổ nào: {body.strip()[:200]}"
    return True, re.search(r"targeting (\S+)", body).group(1)


@check("install")
def _(body: str, expect: str) -> tuple[bool, str]:
    """Cài thật vào đĩa trống phải hoàn tất với mã thoát 0."""
    if not body:
        return False, "không có kết quả — bản cài có thể còn đang chạy, hoặc treo"
    m = re.search(r"^INSTALL_RC=(\d+)", body, re.M)
    if m is None:
        return False, f"không có INSTALL_RC: {body.strip()[:200]}"
    if m.group(1) != "0":
        tail = body.strip()[-400:]
        return False, f"cài đặt trả mã {m.group(1)}: …{tail}"
    started = len(re.findall(r"^▶ ", body, re.M))
    finished = len(re.findall(r"^■ ", body, re.M))
    if started == 0:
        return False, "không có pha nào bắt đầu — nhìn phần đầu kết quả"
    if finished < started:
        return False, f"{started} pha bắt đầu nhưng chỉ {finished} pha xong — cài đặt dừng giữa chừng"
    return True, f"{finished}/{started} pha hoàn tất"


@check("zram")
def _(body: str, expect: str) -> tuple[bool, str]:
    """Trên hệ thống ĐÃ CÀI: mọi thứ hứa ở menu phải có thật trên đĩa.

    Hai lần cài trước (0.1.11, 0.1.12) kết thúc "completed with warnings" và
    mọi pha đều đã xong — nhưng zram vẫn không bao giờ bật, ssh vẫn không cài
    gói. Chỉ có đĩa đích mới kể sự thật, nên khách mở đích lên và trỏ từng thứ:
    unit file, symlink enable, binary/generator, /etc/systemd/zram-generator.conf,
    sshd + openssh.
    """
    if not body:
        return False, "không có kết quả — bước 6 chưa gửi (cài xong rồi mới gửi), có thể đã treo"
    found = dict(re.findall(r"^([a-z_]+)=(yes|no)", body, re.M))
    need = ["zram_unit", "zram_wants", "zram_bin", "zram_conf",
            "sshd_unit", "sshd_wants", "openssh"]
    missing_keys = [k for k in need if k not in found]
    if missing_keys:
        return False, f"thiếu cột mốc {', '.join(missing_keys)} — khách có thể chết giữa chừng: {body.strip()[:200]}"
    bad = [k for k, v in found.items() if v == "no"]
    if bad:
        return False, f"trên đĩa đích còn thiếu: {', '.join(bad)}"
    return True, f"{len(need)} thứ đều có thật trên hệ đã cài"


#: Kiểm tra chỉ chạy khi bài kiểm tra thực sự cài desktop. Profile console thì
#: không có display manager, gói wayland hay cups — đòi những thứ đó sẽ bắt
#: bài kiểm tra console-only phải thất bại, nên nó thuộc về nhánh riêng.
DESKTOP_CHECKS: list[tuple[str, str, object]] = []


def desktop_check(step: str):
    def wrap(fn):
        DESKTOP_CHECKS.append((step, fn.__doc__ or "", fn))
        return fn
    return wrap


@desktop_check("desktop")
def _(body: str, expect: str) -> tuple[bool, str]:
    """Display stack đã chọn phải có thật trên đĩa đích.

    Đây là hàng kiểm tra cho những gì menu hứa mà trước đây không ai đo:
    display manager có unit thật + symlink enable thật, greetd có file config
    trỏ đúng compositor, và các gói display server (wayland, xorg-xwayland)
    thực sự nằm trên đĩa. Cài xong mà thiếu bất kỳ thứ nào thì bản cài vẫn báo
    "completed" — đúng kiểu lỗi mà `outcome` không nhìn thấy.
    """
    if not body:
        return False, "không có kết quả — bước 7 chưa gửi, có thể đã treo"
    found = dict(re.findall(r"^([a-z_]+)=(yes|no)", body, re.M))
    # `is-enabled` trả lời thẳng: "enabled" là thật, còn lại (not-found,
    # disabled, static, masked) đều là chưa bật. Đoán vị trí symlink thì sai:
    # greetd dùng Alias=display-manager.service, không có entry nào trong
    # .wants, và bài kiểm tra trước đã báo nhầm là "thiếu".
    enabled = dict(re.findall(r"^en_([a-z0-9@_.-]+)=(.*)$", body, re.M))
    need = ["dm_unit", "dm_alias", "desktop_pkg", "dm_pkg", "wayland", "xwayland"]
    if "greetd" in expect:
        need += ["greetd_conf", "greetd_session"]
        if enabled.get("greetd") != "enabled":
            return False, f"is-enabled greetd = {enabled.get('greetd', '?')!r}, cần 'enabled'"
    if "cups" in body:
        need += ["cups"]
        if enabled.get("cups") != "enabled":
            return False, f"is-enabled cups = {enabled.get('cups', '?')!r}, cần 'enabled'"
    missing_keys = [k for k in need if k not in found]
    if missing_keys:
        return False, f"thiếu cột mốc {', '.join(missing_keys)} — khách có thể chết giữa chừng: {body.strip()[:200]}"
    bad = [k for k, v in found.items() if v == "no" and k in need]
    if bad:
        return False, f"trên đĩa đích còn thiếu: {', '.join(bad)}"
    return True, f"{len(need)} mốc display có thật + is-enabled xác nhận"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--results", required=True)
    ap.add_argument("--expect-version", default="0.1.13")
    ap.add_argument("--expect-desktop", default="",
                    help="profile desktop của bài kiểm tra (bỏ trống = console-only)")
    args = ap.parse_args()

    results = pathlib.Path(args.results)
    if not results.is_dir():
        print(f"  không có thư mục kết quả: {results}", file=sys.stderr)
        return 1

    # Nếu khảo sát được gửi về, in nó ra: khi có bước hỏng, đây là thứ cho
    # biết máy khách thấy ổ nào.
    survey = results / "survey.json"
    if survey.exists():
        try:
            devices = json.loads(survey.read_text(encoding="utf-8"))
            print(f"  khảo sát thấy {len(devices)} ổ:")
            for d in devices:
                print(f"    {d.get('path'):16} {d.get('size_bytes', 0) / 2**30:6.1f} GiB"
                      f"  bảng phân vùng={d.get('partition_table')}")
        except (json.JSONDecodeError, OSError) as exc:
            print(f"  khảo sát không đọc được: {exc}")

    checks = list(CHECKS)
    if args.expect_desktop:
        checks += DESKTOP_CHECKS

    failures = 0
    print()
    for step, doc, fn in checks:
        body = text(results, step)
        try:
            ok, detail = fn(body, args.expect_version)
        except Exception as exc:                       # noqa: BLE001
            ok, detail = False, f"bộ kiểm tra lỗi: {exc!r}"
        if ok:
            print(f"  \033[32mPASS\033[0m {step:16} {detail}")
        else:
            failures += 1
            print(f"  \033[31mFAIL\033[0m {step:16} {detail}")

    print()
    if failures:
        print(f"  {failures}/{len(checks)} bước hỏng")
        return 1
    print(f"  {len(checks)}/{len(checks)} bước đạt")
    return 0


if __name__ == "__main__":
    sys.exit(main())
