# @wenker-ai/camarelo

**camareLOL 1.0.1 — second beta.** A safety-first TUI installer for Arch Linux,
with a Vietnamese soul.

> The npm `latest` tag is still **0.1.22**. Install this beta with
> `@wenker-ai/camarelo@beta`.

camareLOL installs Arch Linux from the terminal without making you type dozens
of shell commands. It detects your hardware, proposes a safe layout, shows you
exactly what it is about to do, and writes to a disk only after you confirm.

**It is not a shell wrapper.** The installer has its own pipeline — discovery →
plan → review → confirm → execute → verify — with a safety gate in front of
every destructive operation.

- Repository: <https://github.com/tamkogay9876-code/camareLOL>
- Release notes: <https://github.com/tamkogay9876-code/camareLOL/blob/main/docs/releases/1.0.1.md>
- First beta: <https://github.com/tamkogay9876-code/camareLOL/blob/main/docs/releases/1.0.0.md>
- Documentation: <https://github.com/tamkogay9876-code/camareLOL/tree/main/docs>

## Install

On Arch, you need `node`, `npm`, `curl` and `git`:

```console
$ sudo pacman -S --needed nodejs npm curl git
$ npm install -g @wenker-ai/camarelo@beta
```

`npm i -g @wenker-ai/camarelo@beta` is the short form.

For the stable channel (0.1.22):

```console
$ npm install -g @wenker-ai/camarelo
```

On **archiso** (the live ISO) the pacman database is empty, so plain
`pacman -S` reports `database file for 'core' does not exist`. Connect to the
network first, then use:

```console
$ sudo pacman -Sy --needed nodejs npm curl git
```

Then:

```console
$ camarelol --version      # which build is installed
$ camarelol --self-test    # non-destructive checks; safe on a real machine
$ camarelol                # the TUI
```

Installing for real still requires the safety gate:

```console
$ sudo camarelol --allow-destructive --execute
```

and a typed confirmation phrase (`ERASE` or `ERASE AND WIPE`) on the Review
screen. `--preview` shows the complete plan and writes nothing.

## What changed in 1.0.1

`lsblk` labels `zram0`, `ram0`, `nbd0`, `md0` and `loop0` as type `disk`.
camareLOL trusted that label and listed them on the Storage screen: on the
packaging host, `/dev/zram0` appeared as a 4 GiB SATA SSD. `--self-test` then
picked zram as the empty disk, failed the 8 GiB minimum, and reported FAIL on
a host where nothing was wrong. A zram large enough to pass the planner's
minimum would have been a target for an install into volatile memory.

1.0.1 filters those virtual block devices at discovery, and `--self-test` runs
its simulated install on the empty disk it found — or prints
`skipped (no empty disk)` instead of running it against the installed system.
Real disks, virtio (`vd*`, `xv*`), NVMe, MMC and USB devices are unchanged.

The installer itself is otherwise the same code as 1.0.0 — which was 0.1.22
re-versioned and re-documented after a complete roadmap audit. It includes:

- GPT/MBR planning, ext4/btrfs/xfs, firmware-aware validation.
- Minimal Arch install: pacstrap, fstab, GRUB or systemd-boot, locale, keymap,
  timezone, hostname, users, sudo, services, initramfs.
- Seven desktop profiles, Wayland/X11 checks, SDDM/GDM/LightDM/greetd.
- PipeWire/PulseAudio, Bluetooth, zram, swap, NetworkManager.
- Package search, AUR support built as a normal user, saved profiles.
- Three languages: Vietnamese, English, Simplified Chinese.
- Keep-everything dual-boot: existing partitions stay untouched, the ESP is
  reused, new partitions are appended inside a free gap. Real-machine install
  and first boot confirmed on 24 October 2026.

The release notes list the three partial desktop items and the deliberately
deferred work: NTFS resize, real-Windows dual-boot, manual partitioning, LUKS
and Btrfs subvolumes.

## First run asks about updates

The npm package bundles a binary at publish time. If GitHub has a newer
release, the wrapper says so and asks before downloading:

```
═══════════════════════════════════════════════════════════
 The npm build may be older: 1.0.0, GitHub has 1.0.1

 Update? (y/n)
═══════════════════════════════════════════════════════════
```

- **y** — download the newer build from GitHub Releases, verify it against the
  release `SHA256SUMS`, and replace the bundled copy.
- **n** — skip and continue with the bundled build.

With no network, it asks nothing. The Rust binary itself has no updater, no
index URL and no code path that replaces itself; that is deliberate. The
prompt lives in the npm wrapper, before the binary starts, and requires your
`y`.

## About `postinstall`

It asks nothing. `npm install` often runs in CI and Docker builds where nobody
can answer, and a prompt there would hang the build. It only checks that the
machine has what it needs and prints what to install.

## Requirements

| | |
|---|---|
| OS | Arch Linux (or an Arch derivative; the installer itself refuses unvalidated hosts) |
| Architecture | x64 |
| Node | ≥ 18 (only for the npm wrapper) |
| Tools | `curl`, `git`, `sudo`, `pacman` |
| Wi-Fi (optional) | `iwctl` — `sudo pacman -S iwd` |

## Verify the binary

Every release ships `SHA256SUMS`:

```console
$ curl -fLO https://github.com/tamkogay9876-code/camareLOL/releases/download/v1.0.1/camarelol
$ curl -fLO https://github.com/tamkogay9876-code/camareLOL/releases/download/v1.0.1/SHA256SUMS
$ sha256sum -c SHA256SUMS
```

The npm pack process performs the same check before it packs: the binary is
downloaded from the exact GitHub tag in `package.json`, compared with
`SHA256SUMS`, and required to report the same version. A mismatch aborts the
pack.

## License

GPL-3.0-or-later. Source: <https://github.com/tamkogay9876-code/camareLOL>
