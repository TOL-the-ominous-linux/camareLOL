#!/usr/bin/env python3
"""Kiểm tra workflow khởi động được trên GitHub hay không.

    python scripts/check_workflows.py

`yaml.safe_load` chấp nhận một tệp mà GitHub từ chối. Cụ thể: một bước có
`- name:` mà không có `run:` hay `uses:` hợp lệ với Python — `name` chỉ là một
khoá bình thường — nhưng GitHub báo "Required property is missing: run" và
không khởi động job nào cả.

Lần đó toàn bộ workflow hỏng trước khi bước nào chạy: zero job, run báo
failure, mọi thứ trông như đã xong. Không có log nào để đọc.

Nên kiểm tra ở đây, trước khi đẩy.
"""

from __future__ import annotations

import pathlib
import sys

import yaml

ROOT = pathlib.Path(__file__).resolve().parent.parent
WORKFLOWS = ROOT / ".github" / "workflows"

#: `on` là khoá hợp lệ; YAML 1.1 đọc nó thành True, và đó không phải lỗi.
BOOLEANS = {True, False}


def check(path: pathlib.Path) -> list[str]:
    problems: list[str] = []
    try:
        doc = yaml.safe_load(path.read_text(encoding="utf-8"))
    except yaml.YAMLError as exc:
        return [f"{path.name}: YAML không phân tích được: {exc}"]

    if not isinstance(doc, dict):
        return [f"{path.name}: không phải bảng YAML"]

    # `on` hoặc `True` — cả hai là "có khoáa".
    has_trigger = any(k in ("on", True) for k in doc)
    if not has_trigger:
        problems.append(f"{path.name}: không có khoá `on`")

    jobs = doc.get("jobs")
    if not isinstance(jobs, dict) or not jobs:
        return problems + [f"{path.name}: không có job nào"]

    for name, job in jobs.items():
        if not isinstance(job, dict):
            problems.append(f"{path.name}: job '{name}' không phải bảng")
            continue

        # `needs` phải chỉ tới job có thật.
        needs = job.get("needs")
        for need in ([needs] if isinstance(needs, str) else (needs or [])):
            if need not in jobs:
                problems.append(f"{path.name}: job '{name}' needs '{need}' không tồn tại")

        steps = job.get("steps")
        if not steps:
            problems.append(f"{path.name}: job '{name}' không có bước nào")
            continue

        for index, step in enumerate(steps, 1):
            if not isinstance(step, dict):
                problems.append(f"{path.name}: {name} bước {index} không phải bảng")
                continue
            # Đây là điều Python không bắt được.
            if not any(k in step for k in ("run", "uses")):
                label = step.get("name", "(không tên)")
                problems.append(
                    f"{path.name}: {name} bước {index} '{label}' không có run: "
                    f"cũng không có uses: — GitHub sẽ không khởi động job nào")
            elif step.get("uses") and step.get("run"):
                problems.append(
                    f"{path.name}: {name} bước {index} có cả uses: và run:")

    return problems


def main() -> int:
    files = sorted(WORKFLOWS.glob("*.yml")) + sorted(WORKFLOWS.glob("*.yaml"))
    if not files:
        print("  không tìm thấy workflow nào", file=sys.stderr)
        return 1

    problems: list[str] = []
    for path in files:
        found = check(path)
        problems += found
        print(f"  {path.name}: {'OK' if not found else str(len(found)) + ' vấn đề'}")

    if problems:
        print()
        for p in problems:
            print(f"    {p}")
        return 1
    print("  mọi workflow khởi động được")
    return 0


if __name__ == "__main__":
    sys.exit(main())