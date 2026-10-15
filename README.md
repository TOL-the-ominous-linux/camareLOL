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
| Trang web, tài liệu, tin phát hành | **<https://camarelol.netlify.app>** |
| Tài liệu | <https://camarelol.netlify.app/docs.html> |
| Trạng thái mirror | <https://camarelol.netlify.app/status.html> |
| Bản phát hành | <https://github.com/tamkogay9876-code/camareLOL/releases> |
| Bàn luận | <https://github.com/tamkogay9876-code/camareLOL/discussions> |
| Báo lỗi | <https://github.com/tamkogay9876-code/camareLOL/issues> |

The website is generated from this repository, so it cannot disagree with the
code. It is also the update mirror: it serves `/latest.json`, which is the
default update address, with GitHub Releases as a fallback.

## Install

### From a release (fastest)

Download the binary from
[Releases](https://github.com/tamkogay9876-code/camareLOL/releases) and run it
from the Arch live ISO:

```console
$ chmod +x camarelol
$ ./camarelol --self-test      # non-destructive checks
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
$ camarelol --update           # download, verify SHA-256, install
$ camarelol --rollback-update  # put the previous binary back
```

It never replaces itself on its own, never while an install is running, and
refuses a download whose checksum does not match. See
[docs/UPDATES.md](docs/UPDATES.md), including what the checksum does *not*
protect against.

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

`releases/latest/download/` chỉ nhận **đúng tên tệp**, nên ở đây phải ghép
`tên-tệp` từ `tên-thẻ`; không có cách nào dùng đường dẫn đó mà không biết
trước phiên bản.

The full VirtualBox acceptance test is documented in
[docs/TESTING.md](docs/TESTING.md), and a hands-on guide for running it
yourself is in [docs/VM-TESTING.md](docs/VM-TESTING.md).

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
  deferred — see [docs/ROADMAP.md](docs/ROADMAP.md)).
- Dual-boot: detection and preservation rules are implemented; the layout UI for
  reusing an existing ESP comes in Stage 3.

## Architecture

Three components, one protocol:

| component | role |
| --- | --- |
| **Rust core** (`core/`, `tui/`) | authoritative: discovery, planning, safety, install |
| **Python** (`python/`) | validation, recommendations, diagnostics, tests |
| **Java** (`java/`) | package catalog, desktop profiles, plugin interface, JSON-RPC |

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) and
[docs/IPC-PROTOCOL.md](docs/IPC-PROTOCOL.md).

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
cargo clippy --workspace --all-targets -- -D warnings
```

Current status: 92 Rust tests, 19 Python tests, 22 Java tests, plus a full
install verified in a VirtualBox VM (see [docs/TESTING.md](docs/TESTING.md)).

## License

GPL-3.0-or-later. See the headers in each file.

## Warning

Installing an operating system and partitioning disks **can destroy data**.
camareLOL asks before every irreversible step, re-identifies the disk before
writing, and never claims success it did not verify. Still: back up anything you
care about before you install anything.
