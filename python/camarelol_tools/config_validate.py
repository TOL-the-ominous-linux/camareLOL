"""Strict validation of camareLOL installation profiles.

The Rust core is authoritative; this module exists so that a broken profile is
caught *before* we ask the core to do anything with it, and so CI can validate
example profiles without building Rust.
"""

from __future__ import annotations

import re
import tomllib
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

SCHEMA_VERSION = 1

SUPPORTED_FILESYSTEMS = {"ext4", "ext2", "btrfs", "xfs"}
SUPPORTED_BOOTLOADERS = {"grub", "systemd-boot"}
SUPPORTED_DESKTOPS = {
    "console",
    "plasma",
    "gnome",
    "xfce",
    "cinnamon",
    "lxqt",
    "hyprland",
    "sway",
}
SUPPORTED_SERVERS = {"auto", "wayland", "x11"}
SUPPORTED_AUDIO = {"none", "pipewire", "pulseaudio"}
WAYLAND_ONLY_DESKTOPS = {"hyprland", "sway"}
DOMAIN_RE = re.compile(r"^[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?(\.[A-Za-z0-9]([A-Za-z0-9-]{0,61}[A-Za-z0-9])?)*$")
USERNAME_RE = re.compile(r"^[a-z0-9][a-z0-9_-]{0,31}$")
LOCALE_RE = re.compile(r"^[a-z]{2,3}_[A-Z]{2,3}\.[A-Za-z0-9-]+$")
TIMEZONE_RE = re.compile(r"^[A-Za-z][A-Za-z0-9_/+-]*$")


@dataclass
class Problem:
    """A validation problem. `fatal=True` means the profile must be rejected."""

    code: str
    message: str
    remediation: str = ""
    fatal: bool = True

    def __str__(self) -> str:  # pragma: no cover - display only
        base = f"[{self.code}] {self.message}"
        if self.remediation:
            base += f" -> {self.remediation}"
        return base


@dataclass
class ValidationResult:
    problems: list[Problem] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return not any(p.fatal for p in self.problems)

    def __bool__(self) -> bool:
        return self.ok

    def merge(self, other: "ValidationResult") -> "ValidationResult":
        self.problems.extend(other.problems)
        return self


def _check_string(profile: dict[str, Any], key: str, result: ValidationResult) -> str:
    value = profile.get(key)
    if not isinstance(value, str) or not value.strip():
        result.problems.append(
            Problem("invalid_config", f"`{key}` must be a non-empty string", "fix the value")
        )
        return ""
    return value


def _check_bool(profile: dict[str, Any], key: str, result: ValidationResult) -> bool:
    value = profile.get(key, False)
    if not isinstance(value, bool):
        result.problems.append(Problem("invalid_config", f"`{key}` must be a boolean"))
        return False
    return value


def validate_profile(profile: dict[str, Any]) -> ValidationResult:
    """Validate a parsed profile. Mirrors the Rust checks, deliberately."""
    result = ValidationResult()

    schema = profile.get("schema_version", 0)
    if schema != SCHEMA_VERSION:
        result.problems.append(
            Problem(
                "unsupported_schema",
                f"schema_version {schema!r} is not supported (expected {SCHEMA_VERSION})",
                "re-save the profile with the current camareLOL",
            )
        )

    firmware = _check_string(profile, "firmware", result)
    if firmware not in {"auto", "uefi", "bios"}:
        result.problems.append(Problem("invalid_config", f"unknown firmware `{firmware}`"))

    fs = _check_string(profile, "filesystem", result)
    if fs and fs not in SUPPORTED_FILESYSTEMS:
        result.problems.append(
            Problem(
                "invalid_config",
                f"filesystem `{fs}` is not supported for /",
                f"use one of {sorted(SUPPORTED_FILESYSTEMS)}",
            )
        )

    bootloader = _check_string(profile, "bootloader", result)
    if bootloader and bootloader not in SUPPORTED_BOOTLOADERS:
        result.problems.append(
            Problem(
                "invalid_config",
                f"unknown bootloader `{bootloader}`",
                f"use one of {sorted(SUPPORTED_BOOTLOADERS)}",
            )
        )

    desktop = _check_string(profile, "desktop", result)
    if desktop and desktop not in SUPPORTED_DESKTOPS:
        result.problems.append(
            Problem(
                "invalid_config",
                f"unknown desktop `{desktop}`",
                f"use one of {sorted(SUPPORTED_DESKTOPS)}",
            )
        )

    server = _check_string(profile, "display_server", result)
    if server and server not in SUPPORTED_SERVERS:
        result.problems.append(Problem("invalid_config", f"unknown display server `{server}`"))
    if server == "x11" and desktop in WAYLAND_ONLY_DESKTOPS:
        result.problems.append(
            Problem(
                "invalid_config",
                f"`{desktop}` cannot run on X11",
                "choose Wayland or a different desktop",
            )
        )

    locale = _check_string(profile, "system_locale", result)
    if locale and not LOCALE_RE.match(locale):
        result.problems.append(
            Problem(
                "invalid_config",
                f"`{locale}` is not a locale (expected like `en_US.UTF-8`)",
            )
        )

    timezone = _check_string(profile, "timezone", result)
    if timezone and not TIMEZONE_RE.match(timezone):
        result.problems.append(Problem("invalid_config", f"`{timezone}` is not a timezone"))

    hostname = _check_string(profile, "hostname", result)
    if hostname and not DOMAIN_RE.match(hostname):
        result.problems.append(Problem("invalid_account", f"hostname `{hostname}` is invalid"))

    users = profile.get("users", [])
    if not isinstance(users, list) or not all(isinstance(u, str) for u in users):
        result.problems.append(Problem("invalid_account", "`users` must be a list of strings"))
    else:
        for u in users:
            if not USERNAME_RE.match(u) or u == "root":
                result.problems.append(
                    Problem("invalid_account", f"username `{u}` is invalid", "lowercase, digits, `_`, `-`")
                )

    admins = profile.get("admin_users", [])
    for a in admins:
        if a not in users:
            result.problems.append(
                Problem("invalid_account", f"admin `{a}` is not in `users`", "add them to users")
            )

    root_login_allowed = _check_bool(profile, "root_login_allowed", result)
    if not root_login_allowed and not admins:
        result.problems.append(
            Problem(
                "invalid_account",
                "root login is disabled and no admin user exists (you would be locked out)",
                "grant sudo to a user or allow root login",
            )
        )

    # Never accept secrets in a profile.
    lowered = {k.lower() for k in profile}
    for banned in ("password", "root_password", "user_password", "luks_passphrase", "token"):
        if banned in lowered:
            result.problems.append(
                Problem(
                    "invalid_config",
                    f"profile contains `{banned}` — secrets are never stored",
                    "remove it; passwords are entered at install time",
                )
            )

    services = profile.get("services", {})
    if isinstance(services, dict):
        audio = services.get("audio", "none")
        if audio not in SUPPORTED_AUDIO:
            result.problems.append(Problem("invalid_config", f"unknown audio stack `{audio}`"))
        if services.get("zram") and services.get("zram_size_mb", 0) == 0:
            result.problems.append(Problem("invalid_config", "zram_size_mb must be > 0 when zram is on"))
    elif services:
        result.problems.append(Problem("invalid_config", "`services` must be a table"))

    packages = profile.get("packages", {})
    if isinstance(packages, dict):
        if packages.get("aur_enabled") and not packages.get("aur_packages"):
            result.problems.append(
                Problem(
                    "aur_enabled_without_packages",
                    "AUR is enabled but no AUR package is selected",
                    fatal=False,
                )
            )
    else:
        result.problems.append(Problem("invalid_config", "`packages` must be a table"))

    return result


def load_and_validate(path: str | Path) -> tuple[dict[str, Any], ValidationResult]:
    path = Path(path)
    if not path.exists():
        return {}, ValidationResult(
            [Problem("invalid_config", f"{path} does not exist", "check the path")]
        )
    try:
        profile = tomllib.loads(path.read_text(encoding="utf-8"))
    except tomllib.TOMLDecodeError as exc:
        return {}, ValidationResult([Problem("invalid_config", f"{path}: {exc}")])
    return profile, validate_profile(profile)


def main(argv: list[str] | None = None) -> int:
    import argparse

    parser = argparse.ArgumentParser(description="validate a camareLOL profile")
    parser.add_argument("profiles", nargs="+")
    args = parser.parse_args(argv)

    failed = 0
    for p in args.profiles:
        profile, result = load_and_validate(p)
        if result.ok:
            print(f"OK    {p}")
        else:
            failed += 1
            print(f"FAIL  {p}")
            for problem in result.problems:
                print(f"      {problem}")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
