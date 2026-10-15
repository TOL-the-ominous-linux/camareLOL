# Architecture

[← Documentation index](index.md)

camareLOL is built from three components with one rule between them: **only the
Rust core may change the system.** The other two may ask.

```
┌──────────────────────────────────────────────────────────────────┐
│                        camareLOL                                 │
│                                                                    │
│   ┌────────────┐   ┌──────────────┐   ┌───────────────────────┐  │
│   │  TUI       │   │  Python      │   │  Java catalog         │  │
│   │  (ratatui) │   │  tools       │   │  (package catalog,    │  │
│   │            │   │              │   │   desktop profiles,   │  │
│   │  renders,  │   │  validates,  │   │   plugin interface,   │  │
│   │  navigates │   │  recommends, │   │   JSON-RPC server)    │  │
│   └─────┬──────┘   │  diagnoses   │   └──────────┬────────────┘  │
│         │          └──────┬───────┘              │               │
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

## Responsibilities

| Component | Must do | Must never do |
| --- | --- | --- |
| **Rust core** | hardware and storage discovery, partition planning, filesystem and boot configuration, install orchestration, safety validation, privileged operations | store passwords on disk, run a destructive command without a confirmed gate, trust a device path without re-verifying it |
| **Python** | automation, configuration validation, package recommendation, dependency analysis, diagnostics, test utilities | perform destructive operations, bypass the safety layer, silently disable a feature — a missing runtime must be **reported** |
| **Java** | extensible package catalog, desktop profiles, plugin interfaces, configuration modules over a defined API | perform destructive operations, bypass the safety layer |

## Why a core, and not a script

The alternative design is a shell script that calls `sgdisk` and `mkfs`. It is
simpler to write and impossible to reason about at the moment that matters: the
moment a disk is about to be erased.

Putting every system-changing call behind one component means the checks cannot
be forgotten by a caller. Python and Java are faster to add features to and are
therefore more likely to grow new code — so the core validates every request
from them the same way it validates one from the TUI.

## Build order

1. `core/` — the Rust library: `cargo build -p camarelol-core`
2. `tui/` — the `camarelol` binary, which depends on the core
3. `python/` — no build step: `python3 -m unittest discover -s tests`
4. `java/` — `cd java && ./build.sh` (javac, jar, tests)

`./build.sh` at the repository root does all four.

## The IPC protocol

One-line summary: newline-delimited JSON with request ids, a protocol version,
structured typed errors, per-request timeouts and cancellation. See
[the IPC protocol](ipc-protocol.md).

The detail that matters: the core validates **every** request. A Python tool
cannot reach a write path without the core agreeing to it.

## Failure handling

- A failed external tool is reported with its exit code and a sanitised stderr
  excerpt. Secrets are redacted before anything is written.
- An install stops at the **first** failed phase. It never continues into an
  unsafe step, never claims success it did not achieve, and never re-runs a
  destructive step automatically. `recovery::resume_plan` lists which phases are
  safe to retry.
- A disk whose identity changed since the plan was created aborts the run.
- A missing runtime is reported: the installer says "Python tool not available"
  rather than pretending the feature does not exist.

## Security model

1. **No shell string interpolation.** User input never reaches a shell. Every
   external tool is invoked with separate argv entries, and `sfdisk` scripts
   are piped on stdin rather than passed as arguments.
2. **Safety gate.** A destructive operation needs `--allow-destructive` **and** a
   typed confirmation phrase matching its risk level (`ERASE`, `ERASE AND WIPE`).
3. **Device identity.** A plan stores WWN and serial, not `/dev/sda`. Right
   before every write the disk is re-scanned and compared against what was
   planned.
4. **No secrets at rest.** Passwords live in memory only. They are piped to
   `chpasswd` on stdin and never appear in argv, logs or configuration files.
5. **Dry-run by default.** `--execute` is required for any write.

## Testing strategy

| Layer | How |
| --- | --- |
| Unit | `cargo test`, Python `unittest`, Java `CatalogTest` |
| Acceptance | synthetic device surveys run against the built binary via `--mock` |
| Full system | a VirtualBox VM with an empty disk, booted from the Arch ISO, running the installer in scripted mode |
| Packaging | CI builds the release tarball and runs `makepkg` on it in an Arch container, so a broken `PKGBUILD` fails the build |

Details in [testing](testing.md).

## Next

- [The IPC protocol](ipc-protocol.md)
- [Testing](testing.md)
- [What is not built yet](roadmap.md)