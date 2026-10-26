# camareLOL

> A safety-first TUI installer for Arch Linux, in three languages.

**Current release:** `1.0.1` — beta (GitHub pre-release). The npm `latest` tag
deliberately stays on `0.1.22`; the beta is `@wenker-ai/camarelo@beta`.
Read the [1.0.1 release notes](../releases/1.0.1.md). The first beta's notes
remain at [1.0.0](../releases/1.0.0.md).

camareLOL installs Arch Linux from the terminal. It detects your hardware,
proposes a layout, shows you exactly what it is about to do, and only writes to
a disk after you type a confirmation phrase.

**It is not a shell wrapper.** The installer has its own pipeline — discovery →
plan → review → confirm → execute → verify — with a safety gate in front of every
destructive operation.

| | |
|---|---|
| Releases | <https://github.com/tamkogay9876-code/camareLOL/releases> |
| Discussions | <https://github.com/tamkogay9876-code/camareLOL/discussions> |
| Issues | <https://github.com/tamkogay9876-code/camareLOL/issues> |

## Languages

The installer itself speaks Vietnamese, English and Simplified Chinese (`--lang vi|en|zh`).

## Install

Run it from the Arch live ISO, which is where an installer belongs. The default
one-liner resolves the newest **stable** release (0.1.22); the 1.0.1 beta is
opt-in:

```console
$ curl -fsSL https://raw.githubusercontent.com/tamkogay9876-code/camareLOL/main/scripts/camarelol.sh \
    | CAMARELOL_VERSION=v1.0.1 bash
$ camarelol --self-test      # non-destructive checks; safe on a real machine
$ sudo camarelol             # the TUI
```

Use `CAMARELOL_CHANNEL=beta` instead of `CAMARELOL_VERSION` to follow the newest
pre-release. Omit both for the stable channel.

`--self-test` never writes to a disk. Run it first.

If you would rather download the binary yourself, keep `-f` and `-L`:

```console
$ curl -fLO https://github.com/tamkogay9876-code/camareLOL/releases/download/v1.0.1/camarelol
$ curl -fLO https://github.com/tamkogay9876-code/camareLOL/releases/download/v1.0.1/SHA256SUMS
$ sha256sum -c SHA256SUMS
```

Dropping `-f` lets a proxy's 404 page be saved under the name `camarelol` and
still exit 0; `chmod +x` then succeeds on a text file, and the first run reports
`line 1: Not: command not found`.

camareLOL does not update itself. There is no updater, no index URL, and no
code path in the binary that replaces it.

That is a decision, not an unfinished feature. This program erases disks; a
component whose job is to replace itself is the shortest path from a
compromised server to erased data. There is no `pacman` package and no AUR
package. Re-run the one-liner; it picks the newest **stable** release and
verifies it:

```console
$ curl -fsSL https://raw.githubusercontent.com/tamkogay9876-code/camareLOL/main/scripts/camarelol.sh | bash
```

Or download the release you want from
<https://github.com/tamkogay9876-code/camareLOL/releases> and replace the
binary by hand.

## Documentation

| Topic | English | Tiếng Việt | 中文 |
|---|---|---|---|
| The three components | [EN](architecture.md) | [VI](../vi/architecture.md) | [ZH](../zh/architecture.md) |
| The IPC protocol | [EN](ipc-protocol.md) | [VI](../vi/ipc-protocol.md) | [ZH](../zh/ipc-protocol.md) |
| What gets tested | [EN](testing.md) | [VI](../vi/testing.md) | [ZH](../zh/testing.md) |
| Running it yourself | [EN](vm-testing.md) | [VI](../vi/vm-testing.md) | [ZH](../zh/vm-testing.md) |
| Updating | [update](update.md) | [VI](../vi/update.md) | [ZH](../zh/update.md) |
| What is not built yet | [EN](roadmap.md) | [VI](../vi/roadmap.md) | [ZH](../zh/roadmap.md) |

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

- Run on anything other than Arch Linux with systemd. The distribution and the
  init system are read from `/etc/os-release` and `/proc/1/comm`, and anything
  else is refused at plan time, before a byte is written. **Parabola and
  Hyperbola are GNU/Linux-libre distributions, not Arch** — they use `pacman`
  and drop or replace non-free packages, so a package list that is correct on
  Arch can be missing or renamed there. Artix, Nyarch and `ID_LIKE=arch`
  derivatives such as EndeavourOS are refused for the same reason: they are
  unvalidated hosts, not Arch.
- Format a disk because you selected it in a menu.
- Install AUR packages without an explicit opt-in and a normal-user build.
- Resize NTFS — the acceptance criteria are documented, the feature is deferred.
- Boot a real Windows install from the generated menu entry, or shrink Windows
  from inside Windows. The keep-everything planner, the append-only write path
  and the Windows menu entry are VM-proven; the real-Windows leg is deferred.
- Suggest a GPU driver from the parsed model. `GpuInfo.model` is read and shown;
  a model-keyed driver choice is still partial.

## Installer flow

```
Welcome → Language → Keyboard → Timezone → Storage → Disk mode →
Layout → Filesystem → Accounts → Bootloader → Desktop → Services →
Packages → Review (type the confirmation phrase) → Progress → Result
```

Keyboard only. `↑↓` move, `Enter` select, `Space` toggle, `Tab` next field,
`Esc` back, `F1` help, `F5` re-scan, `F10` review, `Ctrl-C` quit.

## Warning

Installing an operating system and partitioning disks **can destroy data**.
camareLOL asks before every irreversible step, re-identifies the disk before
writing, and never claims success it did not verify.

Still: back up anything you care about before you install anything, and run it
from a live ISO rather than on the machine you are trying to fix.
