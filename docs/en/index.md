# camareLOL

> A safety-first TUI installer for Arch Linux, in three languages.

camareLOL installs Arch Linux from the terminal. It detects your hardware,
proposes a layout, shows you exactly what it is about to do, and only writes to
a disk after you type a confirmation phrase.

**It is not a shell wrapper.** The installer has its own pipeline — discovery →
plan → review → confirm → execute → verify — with a safety gate in front of every
destructive operation.

| | |
|---|---|
| Website | **<https://tamkogay9876-code.github.io/camareLOL-mirror>** |
| Releases | <https://github.com/tamkogay9876-code/camareLOL/releases> |
| Discussions | <https://github.com/tamkogay9876-code/camareLOL/discussions> |
| Issues | <https://github.com/tamkogay9876-code/camareLOL/issues> |

## Languages

| Language | Documentation |
|---|---|
| English | <https://tamkogay9876-code.github.io/camareLOL-mirror/en/docs/> |
| Tiếng Việt | <https://tamkogay9876-code.github.io/camareLOL-mirror/vi/docs/> |
| 中文 | <https://tamkogay9876-code.github.io/camareLOL-mirror/zh/docs/> |

The installer itself speaks Vietnamese and English (`--lang vi|en`).

## Install

Run it from the Arch live ISO, which is where an installer belongs:

```console
$ curl -sSLO https://github.com/tamkogay9876-code/camareLOL/releases/latest/download/camarelol
$ chmod +x camarelol
$ ./camarelol --self-test      # non-destructive checks; safe on a real machine
$ sudo ./camarelol             # the TUI
```

`--self-test` never writes to a disk. Run it first.

Staying current — the updater reports by default and installs only when asked:

```console
$ camarelol --check-update     # reports, downloads nothing, exit code 10 if newer
$ camarelol --update           # download, verify signature and SHA-256, install
$ camarelol --rollback-update  # put the previous binary back
```

It never replaces itself on its own, never while an install is running, and
refuses a download whose checksum or signature does not match.

## Documentation

| Topic | English | Tiếng Việt | 中文 |
|---|---|---|---|
| The three components | [architecture](architecture.md) | [VI](https://tamkogay9876-code.github.io/camareLOL-mirror/vi/docs/architecture.html) | [ZH](https://tamkogay9876-code.github.io/camareLOL-mirror/zh/docs/architecture.html) |
| The IPC protocol | [ipc-protocol](ipc-protocol.md) | [VI](https://tamkogay9876-code.github.io/camareLOL-mirror/vi/docs/ipc-protocol.html) | [ZH](https://tamkogay9876-code.github.io/camareLOL-mirror/zh/docs/ipc-protocol.html) |
| What gets tested | [testing](testing.md) | [VI](https://tamkogay9876-code.github.io/camareLOL-mirror/vi/docs/testing.html) | [ZH](https://tamkogay9876-code.github.io/camareLOL-mirror/zh/docs/testing.html) |
| Running it yourself | [vm-testing](vm-testing.md) | [VI](https://tamkogay9876-code.github.io/camareLOL-mirror/vi/docs/vm-testing.html) | [ZH](https://tamkogay9876-code.github.io/camareLOL-mirror/zh/docs/vm-testing.html) |
| What is not built yet | [roadmap](roadmap.md) | [VI](https://tamkogay9876-code.github.io/camareLOL-mirror/vi/docs/roadmap.html) | [ZH](https://tamkogay9876-code.github.io/camareLOL-mirror/zh/docs/roadmap.html) |

## What it does today

- Detects disks, partitions, filesystems, EFI partitions, Windows, recovery and
  Microsoft-reserved partitions, and live-system mounts.
- Plans GPT and MBR layouts: ESP, BIOS boot, `/`, `/home`, swap, with ext4,
  btrfs or xfs, validated against the firmware mode.
- Detects Windows and refuses to touch it. A dual-boot layout is planned, never
  executed blindly.
- Installs a minimal Arch system: pacstrap, fstab, bootloader, locale, keymap,
  timezone, hostname, users, sudo policy, services, initramfs.
- Safety gate: locked by default. `--allow-destructive` plus a typed
  confirmation phrase (`ERASE`, `ERASE AND WIPE`) arms it; `--execute` is
  required before anything is written.
- Configuration profiles: save and load, schema-versioned, **never containing
  passwords**.
- Logs every phase with secret redaction.

## What it deliberately does not do yet

- Format a disk because you selected it in a menu.
- Install AUR packages without an explicit opt-in and a normal-user build.
- Resize NTFS — the acceptance criteria are documented, the feature is deferred.
- Reuse an existing ESP for dual-boot. The detection and preservation rules are
  implemented; the layout interface comes in Stage 3.

## Installer flow

```
Welcome → Language → Keyboard → Timezone → Storage → Disk mode →
Layout → Filesystem → Accounts → Bootloader → Desktop → Services →
Packages → Review (type the confirmation phrase) → Progress → Result
```

Keyboard only. `↑↓` move, `Enter` select, `Space` toggle, `Tab` next field,
`Esc` back, `F1` help, `F5` re-scan, `F10` review, `Ctrl-C` quit, `u` check for
updates.

## Warning

Installing an operating system and partitioning disks **can destroy data**.
camareLOL asks before every irreversible step, re-identifies the disk before
writing, and never claims success it did not verify.

Still: back up anything you care about before you install anything, and run it
from a live ISO rather than on the machine you are trying to fix.