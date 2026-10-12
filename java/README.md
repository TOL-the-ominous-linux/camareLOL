# Java catalog component for camareLOL.
#
# Extensible package catalog, desktop profile management, plugin interfaces
# and configuration modules over the versioned JSON protocol.
#
# Build:   ./build.sh          (compiles + runs tests + jar)
# Run:     ./build.sh run      (JSON-RPC server on port 47812)
#
# The Java side is advisory only: it can never perform destructive
# operations. Those live exclusively in the Rust core.

## Layout

```
src/main/java/com/camarelol/catalog/
    PackageCatalog.java   package groups, desktop profiles, AUR policy
    ProfileService.java   resolves a complete package selection
    CatalogPlugin.java    extension point (ServiceLoader)
    Json.java             minimal JSON reader/writer (no dependencies)
    JsonRpcServer.java    NDJSON server, same protocol as the Rust core

src/test/java/com/camarelol/catalog/
    CatalogTest.java      unit tests (run via ./build.sh)
```
