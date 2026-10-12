"""Diagnostics collection.

Produces the same information a user would need to attach to a bug report, in a
form that contains no secrets (see logging.redact in the Rust core).
"""

from __future__ import annotations

import json
import platform
import re
import shutil
import subprocess
from dataclasses import dataclass, field
from pathlib import Path

SECRET_KEY = re.compile(r"(password|passwd|secret|token|key|luks|apikey)", re.IGNORECASE)


def redact(text: str) -> str:
    """Scrub potential secrets from a diagnostics string."""

    def scrub(match: re.Match[str]) -> str:
        key = match.group(1)
        return f"{key}=<redacted>"

    return re.sub(r"(?i)(password|passwd|secret|token|key|luks|apikey)\s*=\s*\S+", scrub, text)


@dataclass
class Diagnostics:
    python: str
    kernel: str
    arch: str
    tools: dict[str, bool]
    block_devices: list[dict] = field(default_factory=list)
    mounts: list[str] = field(default_factory=list)

    def to_dict(self) -> dict:
        return {
            "python": self.python,
            "kernel": self.kernel,
            "arch": self.arch,
            "tools": self.tools,
            "block_devices": self.block_devices,
            "mounts": self.mounts,
        }

    def to_json(self) -> str:
        return json.dumps(redact(json.dumps(self.to_dict())), indent=2)


REQUIRED_TOOLS = [
    "lsblk",
    "sfdisk",
    "sgdisk",
    "mkfs.ext4",
    "mkfs.vfat",
    "mkswap",
    "partprobe",
    "mount",
    "pacstrap",
    "genfstab",
    "arch-chroot",
]


def tool_availability() -> dict[str, bool]:
    return {tool: shutil.which(tool) is not None for tool in REQUIRED_TOOLS}


def lsblk_survey() -> list[dict]:
    try:
        out = subprocess.run(
            ["lsblk", "-J", "-b", "-o", "NAME,PATH,TYPE,SIZE,FSTYPE,PARTTYPENAME,MOUNTPOINTS"],
            capture_output=True,
            text=True,
            timeout=20,
            check=True,
        ).stdout
    except (OSError, subprocess.SubprocessError):
        return []
    try:
        data = json.loads(out)
    except json.JSONDecodeError:
        return []
    return data.get("blockdevices", [])


def mounts() -> list[str]:
    try:
        with open("/proc/mounts", encoding="utf-8") as fh:
            return [line.split()[1] for line in fh if len(line.split()) > 1]
    except OSError:
        return []


def collect() -> Diagnostics:
    return Diagnostics(
        python=platform.python_version(),
        kernel=platform.release(),
        arch=platform.machine(),
        tools=tool_availability(),
        block_devices=lsblk_survey(),
        mounts=mounts(),
    )


def main(argv: list[str] | None = None) -> int:
    import argparse

    parser = argparse.ArgumentParser(description="collect a camareLOL diagnostics report")
    parser.add_argument("-o", "--output", help="write the report here")
    args = parser.parse_args(argv)

    report = collect().to_json()
    if args.output:
        Path(args.output).write_text(report + "\n", encoding="utf-8")
        print(f"written to {args.output}")
    else:
        print(report)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
