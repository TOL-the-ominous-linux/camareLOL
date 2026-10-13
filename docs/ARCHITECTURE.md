# camareLOL — Architecture

camareLOL is a TUI installer for Arch Linux built from three intentional
components. Each has a strict responsibility and none may bypass the safety
layer.

```
┌──────────────────────────────────────────────────────────────────┐
│                        camareLOL 0.1.0                            │
│                                                                    │
│   ┌────────────┐   ┌──────────────┐   ┌───────────────────────┐  │
│   │  TUI       │   │  Python      │   │  Java catalog         │  │
│   │  (ratatui) │   │  tools       │   │  (package catalog,    │  │
│   │            │   │              │   │   desktop profiles,   │  │
│   │  renders,  │   │  validates,  │   │   plugin interface,   │  │
│   │  navigates │   │  recommends, │   │   JSON-RPC server)    │  │
│   └─────┬──────┘   │  diagnoses   │   └──────────┬────────────┘  │
│         │          └──────┬───────┘              │               │
│         │                 │                      │               │
│         └────────┬────────┴──────────────────────┘               │
│                  │ versioned JSON protocol (NDJSON)              │
│                  ▼                                                │
│   ┌──────────────────────────────────────────────────────────┐  │
│   │              Rust core (authoritative)                    │  │
│   │  hardware · storage · layout · boot · accounts · packages │  │
│   │  config · logging · recovery · install orchestration      │  │
│   │  safety gate — every system-changing request passes here  │  │
│   └──────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘
```

## 1. Responsibilities

| Component | Must do | Must never do |
| --- | --- | --- |
| **Rust core** | hardware/storage discovery, partition planning, filesystem and boot configuration, install orchestration, safety validation, privileged system operations | store passwords on disk, run a destructive command without a confirmed gate, trust a device path without re-verification |
| **Python** | automation, configuration validation, package recommendation, dependency analysis, hardware information processing, diagnostics, test utilities | perform destructive operations, bypass the safety layer, silently disable features (missing runtimes must be *reported*) |
| **Java** | extensible package catalog, desktop profile management, plugin interfaces, configuration modules over a well-defined API | perform destructive operations, bypass the safety layer |

## 2. Build order

1. `core/` — the Rust library (`cargo build -p camarelol-core`).
2. `tui/` — the `camarelol` binary (depends on the core).
3. `python/` — `python3 -m unittest discover -s tests` (no build step).
4. `java/` — `cd java && ./build.sh` (javac + jar + tests).

`./build.sh` at the repository root does all of the above.

## 3. IPC protocol

See [IPC-PROTOCOL.md](IPC-PROTOCOL.md). One-line summary: newline-delimited
JSON with request ids, a protocol version, structured typed errors, timeouts
and cancellation. The Rust core validates **every** request, so Python and
Java can never reach a write path without the core's approval.

## 4. Failure handling

* A failed external tool is reported with its exit code and a sanitized
  stderr excerpt (passwords are redacted).
* An install stops at the first failed phase. It never continues into an
  unsafe step, never claims success it did not achieve, and never re-runs a
  destructive step automatically. `recovery::resume_plan` lists the phases
  that are safe to retry.
* A disk whose identity changed since the plan was created aborts the run.
* Missing runtimes are reported: the installer says "Python tool not
  available" instead of pretending the feature does not exist.

## 5. Security model

1. **No shell string interpolation.** User input never reaches a shell. Every
   external tool is invoked with separate argv entries; sfdisk scripts are
   piped on stdin.
2. **Safety gate.** Destructive operations require
   `--allow-destructive` **and** a typed confirmation phrase that matches the
   risk level of the operation (`ERASE` / `ERASE AND WIPE`).
3. **Device identity.** Plans store WWN/serial, not `/dev/sda`. Right before
   every write the disk is re-scanned and compared.
4. **No secrets at rest.** Passwords live in memory only; they are piped to
   `chpasswd` on stdin, never appear in argv, logs, or configuration files.
5. **Default is dry-run.** `--execute` is required for any write.

## 6. Testing strategy

* Unit tests: Rust (`cargo test`), Python (`unittest`), Java (`CatalogTest`).
* Acceptance criteria: `python3 -m camarelol_tools.testutil` runs the spec
  scenarios (empty disk, Windows disk, partition without filesystem) against
  the built binary using `--mock` surveys.
* Full-system test: a VirtualBox VM with an empty virtual disk, booted from the
  Arch ISO, running the installer in scripted mode. See
  [TESTING.md](TESTING.md).
* Packaging: CI builds the release tarball and runs `makepkg` on it in an
  Arch container, so a broken `PKGBUILD` fails the build.