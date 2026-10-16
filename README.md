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
| Releases | <https://github.com/tamkogay9876-code/camareLOL/releases> |
| Discussions | <https://github.com/tamkogay9876-code/camareLOL/discussions> |
| Issues | <https://github.com/tamkogay9876-code/camareLOL/issues> |

There is no website. The documentation is Markdown in `docs/`, which GitHub
renders in the repository itself — no server to run, nothing to break.

## Documentation

Written in three languages. Every page exists in all three, so each page's
table points sideways into the other two.

| Topic | English | Tiếng Việt | 中文 |
|---|---|---|---|
| Overview | [index](docs/en/index.md) | [VI](docs/vi/index.md) | [ZH](docs/zh/index.md) |
| The three components | [architecture](docs/en/architecture.md) | [VI](docs/vi/architecture.md) | [ZH](docs/zh/architecture.md) |
| The IPC protocol | [ipc-protocol](docs/en/ipc-protocol.md) | [VI](docs/vi/ipc-protocol.md) | [ZH](docs/zh/ipc-protocol.md) |
| What gets tested | [testing](docs/en/testing.md) | [VI](docs/vi/testing.md) | [ZH](docs/zh/testing.md) |
| Running it yourself | [vm-testing](docs/en/vm-testing.md) | [VI](docs/vi/vm-testing.md) | [ZH](docs/zh/vm-testing.md) |
| Updating, and why there is none | [update](docs/en/update.md) | [VI](docs/vi/update.md) | [ZH](docs/zh/update.md) |
| What is not built yet | [roadmap](docs/en/roadmap.md) | [VI](docs/vi/roadmap.md) | [ZH](docs/zh/roadmap.md) |

Start with [the overview](docs/en/index.md).

## What changed in 0.1.11

**The updater is gone.** `--check-update`, `--update` and `--rollback-update`
no longer exist, and there is no index URL, mirror list or signing key left in
the build.

It was never automatic — `--check-update` only reported. What was automatic was
the chain underneath it, and that chain broke four times in one day. The
failure mode was always the same shape: one host stopped answering, another
kept serving a frozen copy of the index, and every camareLOL in the field
reported itself current.

This matters if you are on 0.1.10 or older: your binary has an updater that asks
an address which no longer resolves. It cannot install anything, so nothing is
at risk. Download this release and replace the binary — see
[Updating](docs/en/update.md).

Also removed: the website and its update mirror. Three hosts in one day proved
that a deployment chain you depend on is a liability, not an asset.

The installer, the safety gate, the IPC protocol and the Java catalog are
unchanged.

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

### Via npm

On a running Arch system, npm is one more way in:

```console
$ sudo pacman -S --needed nodejs npm curl
$ npm install -g @wenker-ai/camarelo
```

On the **archiso** (live ISO), the pacman database is empty — use
`sudo pacman -Sy --needed nodejs npm curl` after connecting to the network,
and see [Updating](docs/en/update.md).

The package carries the release binary plus a small Node wrapper that runs
before it. If the bundled build is older than the newest GitHub release, the
wrapper can fetch that release (verified against the release `SHA256SUMS`) and
replace its own copy. The binary never replaces itself — what that does and
does not protect against is written in [Updating](docs/en/update.md).

### Updating

camareLOL does not update itself. There is no updater, no index URL, and no
code path in the binary that replaces it.

That is a decision, not an unfinished feature. This program erases disks; a
component whose job is to replace itself is the shortest path from a compromised
server to erased data. Update it the way every other Arch package is updated:

```console
$ sudo pacman -S camarelol        # from a repository or the AUR
```

Or download the release you want from
[Releases](https://github.com/tamkogay9876-code/camareLOL/releases) and replace
the binary by hand.

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
`Esc` back, `F1` help, `F5` re-scan, `F10` review, `Ctrl-C` quit.

## Testing

```bash
./build.sh test                                # everything (Rust + Python + Java)
cd python && python3 -m unittest discover -s tests -v
cd java && ./build.sh
python scripts/check_lang_table.py              # doc tables point at the right language
python scripts/check_pkgbuild.py                # PKGBUILD matches the newest tag
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