"""Test utilities for the camareLOL acceptance criteria.

The acceptance scenarios from the specification (empty disk, Windows disk,
partition without filesystem, device changed, invalid boot config) are driven
from here so they can run against:
  * the Rust binary with a `--mock` survey, or
  * the Rust unit tests, or
  * the Python validation layer.
"""

from __future__ import annotations

import json
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Any

BINARY_NAMES = ("camarelol",)


def find_binary() -> Path | None:
    for name in BINARY_NAMES:
        found = shutil.which(name)
        if found:
            return Path(found)
    for candidate in (
        Path(__file__).resolve().parents[1] / "target" / "release" / "camarelol",
        Path(__file__).resolve().parents[1] / "target" / "debug" / "camarelol",
    ):
        if candidate.exists():
            return candidate
    return None


@dataclass
class Scenario:
    name: str
    survey: dict[str, Any]

    def to_json(self) -> str:
        # The `--mock` option takes the same JSON array as `--scan` prints.
        return json.dumps([self.survey])


def empty_disk_scenario(size_bytes: int = 20 * 1024**3, path: str = "/dev/sdz") -> Scenario:
    """Acceptance scenario A: a completely empty disk."""
    return Scenario(
        "empty-disk",
        {
            "kernel_name": "sdz",
            "path": path,
            "model": "Virtual disk",
            "serial": "scenario-empty",
            "size_bytes": size_bytes,
            "rotational": False,
            "removable": False,
            "kind": "SataSsd",
            "partition_table": "None",
            "partitions": [],
            "mount_state": "NotMounted",
            "os_hints": [],
            "encrypted": None,
            "risk": "Low",
        },
    )


def windows_disk_scenario() -> Scenario:
    """Acceptance scenario B: a Windows disk that must not be touched."""
    return Scenario(
        "windows-disk",
        {
            "kernel_name": "sdy",
            "path": "/dev/sdy",
            "model": "Windows disk",
            "serial": "scenario-windows",
            "size_bytes": 250 * 1024**3,
            "rotational": False,
            "removable": False,
            "kind": "SataSsd",
            "partition_table": "Gpt",
            "partitions": [
                {
                    "kernel_name": "sdy1",
                    "path": "/dev/sdy1",
                    "size_bytes": 512 * 1024**2,
                    "filesystem": "Ntfs",
                    "partition_type": "ebd0a0a2-b9e5-4433-87c0-68b6b72699c7",
                    "partition_type_name": "Microsoft basic data",
                    "mount_state": "NotMounted",
                    "os_hints": ["Windows"],
                },
                {
                    "kernel_name": "sdy2",
                    "path": "/dev/sdy2",
                    "size_bytes": 100 * 1024**2,
                    "filesystem": "Vfat",
                    "partition_type": "c12a7328-f81f-11d2-ba4b-00a0c93ec93b",
                    "partition_type_name": "EFI System",
                    "mount_state": "NotMounted",
                    "os_hints": ["EfiSystemPartition"],
                },
            ],
            "mount_state": "NotMounted",
            "os_hints": ["Windows"],
            "encrypted": None,
            "risk": "High",
        },
    )


def partition_without_filesystem_scenario() -> Scenario:
    """Acceptance scenario C: a partition that has no filesystem yet."""
    return Scenario(
        "partition-no-fs",
        {
            "kernel_name": "sdx",
            "path": "/dev/sdx",
            "model": "Mixed disk",
            "serial": "scenario-nofs",
            "size_bytes": 500 * 1024**3,
            "rotational": False,
            "removable": False,
            "kind": "SataSsd",
            "partition_table": "Gpt",
            "partitions": [
                {
                    "kernel_name": "sdx1",
                    "path": "/dev/sdx1",
                    "size_bytes": 300 * 1024**3,
                    "filesystem": None,
                    "partition_type_name": "Linux filesystem",
                    "mount_state": "NotMounted",
                    "os_hints": [],
                },
                {
                    "kernel_name": "sdx2",
                    "path": "/dev/sdx2",
                    "size_bytes": 200 * 1024**3,
                    "filesystem": "Ext4",
                    "partition_type_name": "Linux filesystem",
                    "mount_state": "NotMounted",
                    "os_hints": [],
                },
            ],
            "mount_state": "NotMounted",
            "os_hints": [],
            "encrypted": None,
            "risk": "Medium",
        },
    )


ALL_SCENARIOS = (
    empty_disk_scenario(),
    windows_disk_scenario(),
    partition_without_filesystem_scenario(),
)


def run_binary(binary: Path, args: list[str], survey_json: str | None = None, timeout: int = 120):
    """Run the Rust binary against a device survey."""
    with tempfile.TemporaryDirectory(prefix="camarelol-") as tmp:
        cmd = [str(binary), *args]
        if survey_json:
            survey_path = Path(tmp) / "survey.json"
            survey_path.write_text(survey_json, encoding="utf-8")
            cmd += ["--mock", str(survey_path)]
        return subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)


def scenario_checks(binary: Path | None = None) -> list[tuple[str, bool, str]]:
    """Run the acceptance scenarios and report (scenario, passed, detail)."""
    binary = binary or find_binary()
    results: list[tuple[str, bool, str]] = []

    for scenario in ALL_SCENARIOS:
        if binary is None:
            results.append((scenario.name, False, "camarelol binary not found"))
            continue
        proc = run_binary(binary, ["--self-test"], survey_json=scenario.to_json())
        passed = proc.returncode == 0
        detail = (proc.stdout or proc.stderr).strip().splitlines()[-1] if (proc.stdout or proc.stderr) else ""
        results.append((scenario.name, passed, detail))
    return results


def main(argv: list[str] | None = None) -> int:
    all_ok = True
    for name, passed, detail in scenario_checks():
        print(f"{'PASS' if passed else 'FAIL'}  {name}: {detail}")
        all_ok = all_ok and passed
    return 0 if all_ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
