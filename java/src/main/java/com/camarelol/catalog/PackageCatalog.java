package com.camarelol.catalog;

/**
 * The camareLOL package catalog.
 *
 * Extensible: desktop profiles and package groups are plain data, so new
 * entries never require recompiling the Rust core. The catalog is deliberately
 * informative — only the core may order a write to disk.
 */
public final class PackageCatalog {

    public enum Source {
        /** Official repositories: extra, core, community, multilib. */
        OFFICIAL,
        /** Arch User Repository: user-submitted PKGBUILDs, NOT vetted. */
        AUR
    }

    /** One package entry shown to the user. */
    public record PackageEntry(String name, String description, Source source) {
        public static PackageEntry of(String name, String description, Source source) {
            return new PackageEntry(name, description, source);
        }
    }

    /** A named group of packages (checkbox selection in the TUI). */
    public record PackageGroup(String id, String title, PackageEntry[] entries) {
        public static PackageGroup of(String id, String title, PackageEntry... entries) {
            return new PackageGroup(id, title, entries);
        }
    }

    /** Desktop packages shown for a profile. */
    public record DesktopEntry(String profile, String[] packages, String[] displayManagers,
                              String[] displayServers, String[] services, String notes) {
    }

    // ------------------------------------------------------------------
    // The catalog itself. Names match the Arch repos of 2026 and are verified
    // again by the installer at run time; a stale name is reported, never
    // silently "installed".
    // ------------------------------------------------------------------

    public static final PackageGroup[] GROUPS = {
            PackageGroup.of("core", "System packages (always installed)",
                    PackageEntry.of("base", "minimal package set", Source.OFFICIAL),
                    PackageEntry.of("linux", "the kernel", Source.OFFICIAL),
                    PackageEntry.of("linux-firmware", "firmware blobs", Source.OFFICIAL),
                    PackageEntry.of("base-devel", "compilers and build tools", Source.OFFICIAL),
                    PackageEntry.of("sudo", "privilege escalation", Source.OFFICIAL),
                    PackageEntry.of("networkmanager", "network management", Source.OFFICIAL),
                    PackageEntry.of("vim", "text editor", Source.OFFICIAL)),
            PackageGroup.of("utilities", "Utilities",
                    PackageEntry.of("htop", "process viewer", Source.OFFICIAL),
                    PackageEntry.of("btop", "resource monitor", Source.OFFICIAL),
                    PackageEntry.of("git", "version control", Source.OFFICIAL),
                    PackageEntry.of("firefox", "web browser", Source.OFFICIAL),
                    // `p7zip` left the Arch repositories; `7zip` is the package that exists now.
                    PackageEntry.of("7zip", "archiving", Source.OFFICIAL),
                    PackageEntry.of("reflector", "mirror ranking", Source.OFFICIAL)),
            PackageGroup.of("aur", "AUR packages (user-submitted, not vetted)",
                    PackageEntry.of("ibus-bamboo", "Vietnamese input method for IBus", Source.AUR),
                    PackageEntry.of("fcitx5-bamboo", "Vietnamese input method for Fcitx5", Source.AUR),
                    PackageEntry.of("yay", "AUR helper", Source.AUR))
    };

    public static final DesktopEntry[] DESKTOPS = {
            new DesktopEntry("plasma", new String[]{"plasma", "sddm", "konsole", "dolphin"},
                    new String[]{"sddm"}, new String[]{"wayland", "x11"},
                    new String[]{"NetworkManager"}, "full-featured, X11 + Wayland"),
            new DesktopEntry("gnome", new String[]{"gnome", "gdm", "gnome-tweaks"},
                    new String[]{"gdm"}, new String[]{"wayland", "x11"},
                    new String[]{"NetworkManager"}, "Wayland-first"),
            new DesktopEntry("xfce", new String[]{"xfce4", "xfce4-goodies", "lightdm", "lightdm-gtk-greeter"},
                    new String[]{"lightdm"}, new String[]{"wayland", "x11"},
                    new String[]{"NetworkManager"}, "lightweight, classic"),
            new DesktopEntry("cinnamon", new String[]{"cinnamon", "lightdm", "lightdm-gtk-greeter"},
                    new String[]{"lightdm"}, new String[]{"x11"},
                    new String[]{"NetworkManager"}, "X11-oriented"),
            new DesktopEntry("lxqt", new String[]{"lxqt", "sddm"},
                    new String[]{"sddm"}, new String[]{"wayland", "x11"},
                    new String[]{"NetworkManager"}, "very light"),
            new DesktopEntry("hyprland", new String[]{"hyprland", "greetd", "greetd-tuigreet", "kitty", "wofi", "waybar"},
                    new String[]{"greetd"}, new String[]{"wayland"},
                    new String[]{"NetworkManager"}, "Wayland-only tiling"),
            new DesktopEntry("sway", new String[]{"sway", "greetd", "greetd-tuigreet", "foot", "wofi", "waybar"},
                    new String[]{"greetd"}, new String[]{"wayland"},
                    new String[]{"NetworkManager"}, "Wayland-only tiling"),
            new DesktopEntry("console", new String[]{},
                    new String[]{}, new String[]{},
                    new String[]{}, "no graphical environment")
    };

    private PackageCatalog() {
    }

    public static DesktopEntry desktop(String profile) {
        for (DesktopEntry d : DESKTOPS) {
            if (d.profile().equalsIgnoreCase(profile)) {
                return d;
            }
        }
        throw new IllegalArgumentException("unknown desktop profile: " + profile);
    }

    public static PackageGroup[] groups() {
        return GROUPS.clone();
    }
}
