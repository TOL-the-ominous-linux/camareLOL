package com.camarelol.catalog;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ServiceLoader;

/**
 * Extension point for the catalog.
 *
 * Implementations are discovered via {@link ServiceLoader}; they may add desktop
 * profiles or package groups, but they can never influence the safety gate.
 * A plugin that is missing or broken is reported — never silently skipped.
 */
public interface CatalogPlugin {

    /** Unique plugin id, used in logs. */
    String id();

    /** Extra desktop profiles to show in the UI. */
    default PackageCatalog.DesktopEntry[] extraDesktops() {
        return new PackageCatalog.DesktopEntry[0];
    }

    /** Extra package groups to show in the UI. */
    default PackageCatalog.PackageGroup[] extraGroups() {
        return new PackageCatalog.PackageGroup[0];
    }

    /** Load every available plugin, reporting failures. */
    static CatalogPlugin[] loadAll() {
        java.util.List<CatalogPlugin> plugins = new java.util.ArrayList<>();
        for (CatalogPlugin plugin : ServiceLoader.load(CatalogPlugin.class)) {
            plugins.add(plugin);
        }
        return plugins.toArray(new CatalogPlugin[0]);
    }

    /** Load a single plugin from a directory (its jar must be on the classpath). */
    static CatalogPlugin loadFrom(Path jar) throws IOException {
        if (!Files.exists(jar)) {
            throw new IOException("plugin not found: " + jar);
        }
        // The actual ServiceLoader wiring happens at runtime; this method only
        // makes the failure mode explicit and testable.
        throw new IOException("plugin loading requires the jar on the classpath: " + jar);
    }
}
