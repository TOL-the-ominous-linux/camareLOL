package com.camarelol.catalog;

import java.util.List;
import java.util.Map;

/**
 * Unit tests for the catalog, runnable with `java com.camarelol.catalog.CatalogTest`.
 *
 * The Java component is advisory: the assertions below mirror the rules the
 * Rust core enforces, so a regression on either side fails here.
 */
public final class CatalogTest {

    private static int failures;

    public static void main(String[] args) {
        catalogHasCoreGroups();
        desktopProfilesAreComplete();
        plasmaResolution();
        swayIsWaylandOnly();
        consoleHasNoDisplayManager();
        aurNeverPushedImplicitly();
        jsonRoundTrip();
        jsonRejectsGarbage();

        if (failures > 0) {
            System.out.println("CatalogTest: " + failures + " failure(s)");
            System.exit(1);
        }
        System.out.println("CatalogTest: OK");
    }

    // ------------------------------------------------------------------

    private static void check(boolean condition, String label) {
        if (condition) {
            System.out.println("PASS  " + label);
        } else {
            failures++;
            System.out.println("FAIL  " + label);
        }
    }

    private static void catalogHasCoreGroups() {
        PackageCatalog.PackageGroup[] groups = PackageCatalog.groups();
        boolean hasCore = false;
        boolean hasAur = false;
        for (PackageCatalog.PackageGroup g : groups) {
            if ("core".equals(g.id())) {
                hasCore = true;
            }
            if ("aur".equals(g.id())) {
                hasAur = true;
                for (PackageCatalog.PackageEntry e : g.entries()) {
                    check(e.source() == PackageCatalog.Source.AUR,
                            "aur group marks " + e.name() + " as AUR");
                }
            }
        }
        check(hasCore, "catalog has a core group");
        check(hasAur, "catalog has an explicitly-labeled AUR group");
    }

    private static void desktopProfilesAreComplete() {
        check(PackageCatalog.DESKTOPS.length == 8, "eight desktop profiles");
        for (PackageCatalog.DesktopEntry d : PackageCatalog.DESKTOPS) {
            check(d.packages().length > 0 || "console".equals(d.profile()),
                    d.profile() + " declares its packages");
        }
    }

    private static void plasmaResolution() {
        ProfileService.ProfilePlan plan =
                ProfileService.resolve("plasma", "wayland", "sddm", false, new String[0]);
        check(plan.officialPackages().length >= 4, "plasma suggests its packages");
        check(plan.warnings().length == 0, "plasma/wayland produces no warnings");
    }

    private static void swayIsWaylandOnly() {
        try {
            ProfileService.resolve("sway", "x11", null, false, new String[0]);
            check(false, "sway rejects X11");
        } catch (IllegalArgumentException expected) {
            check(true, "sway rejects X11");
        }
    }

    private static void consoleHasNoDisplayManager() {
        ProfileService.ProfilePlan plan =
                ProfileService.resolve("console", "auto", null, false, new String[0]);
        check(plan.displayManager() == null, "console-only never selects a display manager");
        check(plan.warnings().length == 0, "console-only produces no warnings");
    }

    private static void aurNeverPushedImplicitly() {
        ProfileService.ProfilePlan plan =
                ProfileService.resolve("plasma", "wayland", "sddm", true, new String[0]);
        check(plan.aurPackages().length == 0,
                "enabling AUR adds nothing unless a package is explicitly requested");
        check(plan.warnings().length > 0, "AUR opt-in always warns");
    }

    private static void jsonRoundTrip() {
        Map<String, Object> object = new java.util.LinkedHashMap<>();
        object.put("status", "ok");
        object.put("code", 42);
        object.put("flag", true);
        object.put("nothing", null);
        object.put("list", List.of("a", "b"));
        String text = Json.write(object);
        Map<String, Object> back = Json.parseObject(text);
        check("ok".equals(back.get("status")), "json string round trip");
        check(Json.asInt(back.get("code")) == 42, "json number round trip");
        check(Json.asBool(back.get("flag")), "json boolean round trip");
        check(back.get("nothing") == null, "json null round trip");
        check(Json.asArray(back.get("list")).size() == 2, "json array round trip");
    }

    private static void jsonRejectsGarbage() {
        try {
            Json.parse("this is not json");
            check(false, "json parser rejects garbage");
        } catch (Json.JsonException expected) {
            check(true, "json parser rejects garbage");
        }
        try {
            Json.parseObject("[1,2]");
            check(false, "json parser rejects arrays at the top level");
        } catch (Json.JsonException expected) {
            check(true, "json parser rejects arrays at the top level");
        }
    }
}
