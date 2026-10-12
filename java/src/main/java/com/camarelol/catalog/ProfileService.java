package com.camarelol.catalog;

/**
 * Resolves a complete package selection for a desktop profile.
 *
 * The resolution rules are the single source of truth for package names; the
 * TUI simply displays the result. AUR packages are never selected implicitly.
 */
public final class ProfileService {

    /** AUR may only be enabled explicitly. */
    public record ProfilePlan(String desktop, String displayServer, String displayManager,
                             String[] officialPackages, String[] aurPackages,
                             String[] services, String[] warnings) {
    }

    private ProfileService() {
    }

    public static ProfilePlan resolve(String desktop, String displayServer, String displayManager,
                                     boolean aurEnabled, String[] extraPackages) {
        PackageCatalog.DesktopEntry entry = PackageCatalog.desktop(desktop);

        // 1. validate the display server against the profile
        String server = displayServer == null || displayServer.isBlank() ? "auto" : displayServer;
        if (!"auto".equals(server) && !contains(entry.displayServers(), server)) {
            throw new IllegalArgumentException(
                    "display server " + server + " is not supported by " + desktop);
        }

        // 2. validate the display manager
        String dm = displayManager;
        if (dm == null || dm.isBlank()) {
            dm = entry.displayManagers().length > 0 ? entry.displayManagers()[0] : null;
        } else if (dm != null && !contains(entry.displayManagers(), dm)) {
            throw new IllegalArgumentException(
                    "display manager " + dm + " is not supported by " + desktop);
        }

        // 3. assemble the package list (deduplicated, sorted)
        java.util.TreeSet<String> official = new java.util.TreeSet<>();
        for (String p : entry.packages()) {
            official.add(p);
        }
        for (String p : extraPackages) {
            if (p != null && !p.isBlank()) {
                official.add(p.trim());
            }
        }

        // 4. AUR is opt-in only
        java.util.List<String> aur = new java.util.ArrayList<>();
        if (aurEnabled) {
            // only add AUR packages that were explicitly requested elsewhere;
            // the catalog itself never pushes them in.
        }

        java.util.List<String> warnings = new java.util.ArrayList<>();
        if (aurEnabled) {
            warnings.add("AUR packages are built from user-submitted PKGBUILDs, "
                    + "which are not vetted by Arch Linux.");
        }
        if ("console".equals(desktop) && dm != null) {
            warnings.add("a console-only install should not enable a display manager");
        }
        if (contains(aur.toArray(new String[0]), "ibus-bamboo")
                && contains(aur.toArray(new String[0]), "fcitx5-bamboo")) {
            warnings.add("ibus-bamboo and fcitx5-bamboo should not both be installed");
        }

        return new ProfilePlan(
                entry.profile(),
                server,
                dm,
                official.toArray(new String[0]),
                aur.toArray(new String[0]),
                entry.services(),
                warnings.toArray(new String[0]));
    }

    private static boolean contains(String[] values, String needle) {
        if (needle == null) {
            return false;
        }
        for (String v : values) {
            if (needle.equals(v)) {
                return true;
            }
        }
        return false;
    }
}
