# camareLOL

> A modern, safety-first TUI installer for Arch Linux — with a Vietnamese soul.

camareLOL installs Arch Linux from the terminal without making you type dozens
of shell commands. It detects your hardware, proposes a safe layout, shows you
exactly what it is about to do, and only writes to a disk after you confirm it.

**It is not a shell wrapper.** The installer has its own pipeline: discovery →
plan → review → confirm → execute → verify, with a safety gate in front of every
destructive operation.

## Quick start

```bash
# Build everything (needs cargo + rustup, python3, javac)
./build.sh build

# Non-destructive checks on this machine
./target/release/camarelol --self-test

# Real install (TUI)
sudo ./target/release/camarelol
```

For the full VirtualBox acceptance test see [docs/TESTING.md](docs/TESTING.md).

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
./build.sh test                              # everything
python -m unittest discover -s tests -v       # Python only
cd java && ./build.sh                         # Java only
python -m camarelol_tools.testutil            # acceptance scenarios vs the binary
```

## License

GPL-3.0-or-later. See the headers in each file.

## Warning

Installing an operating system and partitioning disks **can destroy data**.
camareLOL asks before every irreversible step, re-identifies the disk before
writing, and never claims success it did not verify. Still: back up anything you
care about before you install anything.
