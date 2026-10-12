"""Hardware information processing.

Reads the same sources as the Rust core (/proc, /sys) so both sides produce the
same answers; used for suggestions and for the installer's preflight screen.
"""

from __future__ import annotations

import platform
from dataclasses import dataclass
from pathlib import Path


@dataclass
class CpuInfo:
    model: str
    vendor: str
    cores: int
    microcode_hint: str  # "intel" | "amd" | "unknown"


@dataclass
class Hardware:
    firmware: str  # "uefi" | "bios" | "unknown"
    cpu: CpuInfo
    ram_bytes: int
    virtualized: bool
    gpus: list[str]


def detect_firmware() -> str:
    if Path("/sys/firmware/efi/efivars").exists():
        return "uefi"
    if Path("/sys/firmware/acpi/tables").exists() or Path("/sys/firmware/devicetree").exists():
        return "bios"
    return "unknown"


def detect_cpu() -> CpuInfo:
    model = "unknown CPU"
    vendor = ""
    core_ids: set[str] = set()
    try:
        raw = Path("/proc/cpuinfo").read_text(encoding="utf-8", errors="replace")
    except OSError:
        raw = ""
    for line in raw.splitlines():
        if ":" not in line:
            continue
        key, value = (part.strip() for part in line.split(":", 1))
        if key == "model name" and model == "unknown CPU":
            model = value
        elif key == "vendor_id" and not vendor:
            vendor = value
        elif key == "core id":
            core_ids.add(value)
    if "Intel" in vendor or "GenuineIntel" in vendor:
        hint = "intel"
    elif "AMD" in vendor or "AuthenticAMD" in vendor:
        hint = "amd"
    else:
        hint = "unknown"
    return CpuInfo(model=model, vendor=vendor, cores=max(len(core_ids), 1), microcode_hint=hint)


def detect_ram_bytes() -> int:
    try:
        raw = Path("/proc/meminfo").read_text(encoding="utf-8", errors="replace")
    except OSError:
        return 0
    for line in raw.splitlines():
        if line.startswith("MemTotal:"):
            kib = int(line.split()[1])
            return kib * 1024
    return 0


def detect_gpus() -> list[str]:
    gpus: list[str] = []
    try:
        import subprocess

        out = subprocess.run(
            ["lspci", "-mm", "-nn"], capture_output=True, text=True, timeout=10, check=True
        ).stdout
    except (OSError, subprocess.SubprocessError):
        return gpus
    for line in out.splitlines():
        low = line.lower()
        if "vga" in low or "3d controller" in low or "display" in low:
            gpus.append(line.strip())
    return gpus


def detect_virtualized() -> bool:
    try:
        import subprocess

        out = subprocess.run(
            ["systemd-detect-virt"], capture_output=True, text=True, timeout=10
        )
        kind = out.stdout.strip()
        return bool(kind) and kind != "none"
    except (OSError, subprocess.SubprocessError):
        return False


def summarize() -> Hardware:
    return Hardware(
        firmware=detect_firmware(),
        cpu=detect_cpu(),
        ram_bytes=detect_ram_bytes(),
        virtualized=detect_virtualized(),
        gpus=detect_gpus(),
    )


def suggested_zram_mb(ram_bytes: int) -> int:
    """Half of RAM, rounded to 128 MiB steps (same rule as the Rust core)."""
    gib = ram_bytes // (1024**3)
    target = (gib * 1024) // 2
    return max(256, (target + 127) // 128 * 128)


def main(argv: list[str] | None = None) -> int:
    import json

    hw = summarize()
    print(
        json.dumps(
            {
                "firmware": hw.firmware,
                "cpu": {
                    "model": hw.cpu.model,
                    "vendor": hw.cpu.vendor,
                    "cores": hw.cpu.cores,
                    "microcode_hint": hw.cpu.microcode_hint,
                },
                "ram_bytes": hw.ram_bytes,
                "virtualized": hw.virtualized,
                "gpus": hw.gpus,
                "kernel": platform.release(),
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
