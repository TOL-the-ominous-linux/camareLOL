# camareLOL

> A modern, safety-first TUI installer for Arch Linux — with a Vietnamese soul.

camareLOL installs Arch Linux from the terminal without making you type dozens
of shell commands. It detects your hardware, proposes a safe layout, shows you
exactly what it is about to do, and only writes to a disk after you confirm it.

**It is not a shell wrapper.** The installer has its own pipeline: discovery →
plan → review → confirm → execute → verify, with a safety gate in front of every
destructive operation.

| | |
|---|---|
| Website | **<https://camarelol.netlify.app>** |
| Releases | <https://github.com/tamkogay9876-code/camareLOL/releases> |
| Discussions | <https://github.com/tamkogay9876-code/camareLOL/discussions> |
| Issues | <https://github.com/tamkogay9876-code/camareLOL/issues> |

The website is generated from this repository, so it cannot disagree with the
code. It is also the update mirror: it serves `/latest.json`, which is the
default update address, with GitHub Releases as a fallback.

## Documentation

Written in three languages. Every page exists in all three; the language
switcher is in the footer of the site.

| Topic | English | Tiếng Việt | 中文 |
|---|---|---|---|
| Overview | [index](docs/en/index.md) | [VI](docs/vi/index.md) | [ZH](docs/zh/index.md) |
| How updates are signed | [updates-signing](docs/en/updates-signing.md) | [VI](docs/vi/updates-signing.md) | [ZH](docs/zh/updates-signing.md) |
| Why an old index is refused | [updates-rollback](docs/en/updates-rollback.md) | [VI](docs/vi/updates-rollback.md) | [ZH](docs/zh/updates-rollback.md) |
| Building your own package | [updates-packaging](docs/en/updates-packaging.md) | [VI](docs/vi/updates-packaging.md) | [ZH](docs/zh/updates-packaging.md) |
| When an update is refused | [updates-troubleshooting](docs/en/updates-troubleshooting.md) | [VI](docs/vi/updates-troubleshooting.md) | [ZH](docs/zh/updates-troubleshooting.md) |
| The three components | [architecture](docs/en/architecture.md) | [VI](docs/vi/architecture.md) | [ZH](docs/zh/architecture.md) |
| The IPC protocol | [ipc-protocol](docs/en/ipc-protocol.md) | [VI](docs/vi/ipc-protocol.md) | [ZH](docs/zh/ipc-protocol.md) |
| What gets tested | [testing](docs/en/testing.md) | [VI](docs/vi/testing.md) | [ZH](docs/zh/testing.md) |
| Running it yourself | [vm-testing](docs/en/vm-testing.md) | [VI](docs/vi/vm-testing.md) | [ZH](docs/zh/vm-testing.md) |
| What is not built yet | [roadmap](docs/en/roadmap.md) | [VI](docs/vi/roadmap.md) | [ZH](docs/zh/roadmap.md) |

Read it on the site instead: <https://camarelol.netlify.app/en/docs/>

## Install

### From a release (fastest)

Download the binary from
[Releases](https://github.com/tamkogay9876-code/camareLOL/releases) and run it
from the Arch live ISO:

```console
$ chmod +x camarelol
$ ./camarelol --self-test      # non-destructive checks; safe on a real machine
$ sudo ./camarelol             # the TUI
```

Or let the helper fetch it for you:

```console
$ git clone https://github.com/tamkogay9876-code/camareLOL
$ cd camareLOL && sudo ./install.sh --binary
```

Staying current — the updater reports by default and installs only when you ask:

```console
$ camarelol --check-update     # reports, downloads nothing
$ camarelol --update           # download, verify signature and SHA-256, install
$ camarelol --rollback-update  # put the previous binary back
```

It never replaces itself on its own, never while an install is running, and
refuses a download whose signature or checksum does not match. See
[docs/en/updates-signing.md](docs/en/updates-signing.md) and
[docs/en/updates-rollback.md](docs/en/updates-rollback.md).

### From source

```console
$ ./install.sh                # builds and installs to /usr/local/bin
$ ./build.sh build            # compile everything, no install
$ ./build.sh test             # unit tests + acceptance scenarios
$ ./build.sh lint             # clippy + rustfmt
```

### From the tarball (`makepkg`)

Each release ships a `PKGBUILD`:

```console
$ VERSION=0.1.10
$ curl -sSLO "https://github.com/tamkogay9876-code/camareLOL/releases/download/v$VERSION/camarelol-$VERSION.tar.gz"
$ tar xf "camarelol-$VERSION.tar.gz" && cd "camarelol-$VERSION"
$ makepkg -si
```

`releases/latest/download/` takes **the exact file name**, so it has to be built
from the tag name; there is no way to use that path without knowing the version
in advance.

The full VirtualBox acceptance test is documented in
[docs/en/testing.md](docs/en/testing.md), and a hands-on guide for running it
yourself is in [docs/en/vm-testing.md](docs/en/vm-testing.md).

## What it does today

- Detects disks, partitions, filesystems, EFI partitions, Windows, recovery and
  Microsoft-reserved partitions, live-system mounts.
- Plans GPT/MBR layouts: ESP, BIOS boot, `/`, `/home`, swap — with filesystem
  choices (ext4, btrfs, xfs) and firmware-aware validation.
- Detects Windows and refuses to touch it; dual-boot layout is planned, never
  executed blindly.
- Installs a minimal Arch system: pacstrap, fstab, bootloader (GRUB/systemd-boot),
  locale, keymap, timezone, hostname, users, sudo policy, services, initramfs.
- Safety gate: locked by default; `--allow-destructive` + typed confirmation
  phrases (`ERASE`, `ERASE AND WIPE`) arm it; `--execute` is required to write.
- Configuration profiles: save/load, schema-versioned, **never containing
  passwords**.
- Signed self-updater with mirror fallback.
- Logs everything with secret redaction; diagnostics report for bug reports.

## What it deliberately does not do yet

- Format a disk just because you selected it in a menu.
- Install AUR packages without an explicit opt-in and a normal-user build.
- Resize NTFS (the acceptance criteria for it are documented but the feature is
  deferred — see [docs/en/roadmap.md](docs/en/roadmap.md)).
- Dual-boot: detection and preservation rules are implemented; the layout UI for
  reusing an existing ESP comes in Stage 3.

## Architecture

Three components, one protocol:

| component | role |
| --- | --- |
| **Rust core** (`core/`, `tui/`) | authoritative: discovery, planning, safety, install |
| **Python** (`python/`) | validation, recommendations, diagnostics, tests |
| **Java** (`java/`) | package catalog, desktop profiles, plugin interface, JSON-RPC |

See [docs/en/architecture.md](docs/en/architecture.md) and
[docs/en/ipc-protocol.md](docs/en/ipc-protocol.md).

## Installer flow

```
Welcome → Language → Keyboard → Timezone → Storage → Disk mode →
Layout → Filesystem → Accounts → Bootloader → Desktop → Services →
Packages → Review (type the confirmation phrase) → Progress → Result
```

Keyboard only: `↑↓` move, `Enter` select, `Space` toggle, `Tab` next field,
`Esc` back, `F1` help, `F5` re-scan, `F10` review, `Ctrl-C` quit, `u` updates.

## Testing

```bash
./build.sh test                                # everything (Rust + Python + Java)
cd python && python3 -m unittest discover -s tests -v
cd java && ./build.sh
python site/test_markdown.py                   # site renderer
python site/test_render_site.py                # site build, offline
python scripts/check_pkgbuild.py               # PKGBUILD matches the newest tag
cargo clippy --workspace --all-targets -- -D warnings
```

A full install is verified in a VirtualBox VM — see
[docs/en/testing.md](docs/en/testing.md).

## License

GPL-3.0-or-later. See the headers in each file.

## Warning

Installing an operating system and partitioning disks **can destroy data**.
camareLOL asks before every irreversible step, re-identifies the disk before
writing, and never claims success it did not verify. Still: back up anything you
care about before you install anything.