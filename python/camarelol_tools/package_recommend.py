"""Package recommendations and dependency analysis.

Pure advisory: the installer shows these as suggestions and never installs
anything the user did not select. AUR packages are always flagged as extra.
"""

from __future__ import annotations

from dataclasses import dataclass, field

# Groups the installer offers. Names mirror the Java catalog
# (java/com/camarelol/catalog) so both sides agree.
CORE_GROUPS = {
    "base_system": ["base", "linux", "linux-firmware", "sudo", "base-devel"],
    "network": ["networkmanager"],
    "editors": ["vim"],
}

UTILITY_GROUPS = {
    "browser": ["firefox"],
    "development": ["git", "base-devel"],
    "monitoring": ["htop", "btop"],
    # `p7zip` was dropped from the Arch repositories; the community fork that
    # took its place is `7zip`. Listing p7zip here meant pacstrap failed on a
    # profile that included it.
    "archiving": ["7zip", "unzip"],
}

AUR_GROUPS = {
    "input_method_vi": ["ibus-bamboo"],
    "extras": ["yay"],
}

KNOWN_CONFLICTS = [
    # (a, b, reason)
    ("pulseaudio", "pipewire-pulse", "both provide a PulseAudio replacement"),
    ("ibus", "fcitx5", "two input-method frameworks fighting over XIM"),
]

DESKTOP_PACKAGES = {
    "console": [],
    "plasma": ["plasma", "sddm", "konsole", "dolphin"],
    "gnome": ["gnome", "gdm", "gnome-tweaks"],
    "xfce": ["xfce4", "xfce4-goodies", "lightdm", "lightdm-gtk-greeter"],
    "cinnamon": ["cinnamon", "lightdm", "lightdm-gtk-greeter"],
    "lxqt": ["lxqt", "sddm"],
    "hyprland": ["hyprland", "greetd", "greetd-tuigreet", "kitty", "wofi", "waybar"],
    "sway": ["sway", "greetd", "greetd-tuigreet", "foot", "wofi", "waybar"],
}

AUDIO_PACKAGES = {
    "none": [],
    "pipewire": ["pipewire", "pipewire-pulse", "pipewire-alsa", "wireplumber"],
    "pulseaudio": ["pulseaudio", "pulseaudio-alsa"],
}


@dataclass
class Recommendation:
    packages: list[str] = field(default_factory=list)
    aur: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)


def _detect_conflicts(selected: list[str]) -> list[str]:
    problems = []
    lowered = [p.lower() for p in selected]
    for a, b, reason in KNOWN_CONFLICTS:
        if a.lower() in lowered and b.lower() in lowered:
            problems.append(f"{a} conflicts with {b}: {reason}")
    return problems


def recommend(
    desktop: str = "console",
    display_server: str = "auto",
    audio: str = "pipewire",
    language: str = "en",
    use_aur: bool = False,
    extra_groups: list[str] | None = None,
) -> Recommendation:
    """Suggest a package set for a profile."""
    rec = Recommendation()
    rec.packages.extend(CORE_GROUPS["base_system"])
    rec.packages.extend(CORE_GROUPS["network"])

    if desktop and desktop != "console":
        rec.packages.extend(DESKTOP_PACKAGES.get(desktop, []))
        if display_server == "wayland" and desktop in {"plasma", "gnome"}:
            rec.packages.append("xdg-desktop-portal")
    rec.packages.extend(AUDIO_PACKAGES.get(audio, []))

    for group in extra_groups or []:
        rec.packages.extend(UTILITY_GROUPS.get(group, []))

    if language.startswith("vi"):
        # Vietnamese input method; only one framework at a time.
        rec.packages.extend(["noto-fonts", "noto-fonts-cjk", "ibus"])
        if use_aur:
            rec.aur.extend(AUR_GROUPS["input_method_vi"])
            rec.warnings.append(
                "ibus-bamboo comes from the AUR: the PKGBUILD is not vetted by Arch Linux"
            )

    if use_aur:
        rec.warnings.append("AUR packages are built as a normal user, never as root")

    rec.packages = sorted(dict.fromkeys(rec.packages))
    rec.aur = sorted(dict.fromkeys(rec.aur))
    rec.warnings.extend(_detect_conflicts(rec.packages))
    return rec


def analyze_dependencies(packages: list[str]) -> dict[str, list[str]]:
    """Check our recommendation sets for conflicts.

    Real dependency resolution needs the pacman database, which is only
    available inside the live environment; this static view catches the
    conflicts we actually ship with.
    """
    warnings = _detect_conflicts(packages)
    return {"conflicts": warnings}


def main(argv: list[str] | None = None) -> int:
    import argparse
    import json

    parser = argparse.ArgumentParser(description="suggest packages for a camareLOL profile")
    parser.add_argument("--desktop", default="console", choices=sorted(DESKTOP_PACKAGES))
    parser.add_argument("--server", default="auto", choices=["auto", "wayland", "x11"])
    parser.add_argument("--audio", default="pipewire", choices=sorted(AUDIO_PACKAGES))
    parser.add_argument("--language", default="en")
    parser.add_argument("--aur", action="store_true")
    parser.add_argument("--group", action="append", default=[])
    args = parser.parse_args(argv)

    rec = recommend(
        desktop=args.desktop,
        display_server=args.server,
        audio=args.audio,
        language=args.language,
        use_aur=args.aur,
        extra_groups=args.group,
    )
    print(
        json.dumps(
            {
                "packages": rec.packages,
                "aur": rec.aur,
                "warnings": rec.warnings,
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
