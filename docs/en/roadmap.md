# Roadmap

[← Documentation index](index.md)

What is built, what is not, and — where it is not — why.

**Current release:** `1.0.3`, the fourth beta (28 October 2026). It fixes a
virtio disk being labelled `SATA HDD` in the storage survey — display only,
with regression tests; planning never read that field. `1.0.2` made
`--self-test` report `SKIP`, exit 2, on an unsupported host instead of FAIL;
`1.0.1` filtered zram/ram/nbd/md/loop out of disk discovery. `1.0.0` was the
first beta: the 0.1.22 installer code plus the roadmap audit and documentation
overhaul. The npm `latest` tag remains on 0.1.22 until a beta is promoted.

## Stage 1 — MVP

Complete.

- [x] TUI framework with keyboard navigation and help
- [x] Environment checks: firmware, CPU, RAM, network
- [x] Storage detection via `lsblk` / `findmnt` / `blkid`, with risk assessment
- [x] Automatic layout for empty disks: GPT and MBR, ESP, BIOS boot, root,
      `/home`, swap
- [x] Filesystem choice: ext4, btrfs, xfs
- [x] Locale, keymap, timezone
- [x] Hostname, root, user accounts, sudo policy with lock-out prevention
- [x] Minimal Arch install: pacstrap, fstab, bootloader, services
- [x] Logging and confirmation screens
- [x] Safety gate: locked, dry-run, execute with a typed phrase
- [x] Configuration save and load, schema-versioned, no passwords
- [x] ~~Signed self-updater with mirror fallback~~ — removed, and this is the
      intended state. Updates come from `pacman` or a manual download. A
      component whose job is to replace a disk-erasing binary is the shortest
      path from a compromised server to erased data, and the mirror chain
      behind it failed repeatedly while reporting success.
- [x] Python automation layer, Java catalog, versioned IPC

## A stage is only ticked with evidence

Every `[x]` above was checked against a real VirtualBox install, not against
the presence of a struct. Four package names passed the entire unit suite and
only failed in a VM — `greetd-tuigreeter` (Hyprland and Sway could not install),
`p7zip` (removed from the Arch repositories), and `systemd-boot` (not a
package at all; every systemd-boot install died at `pacstrap`). A checkbox that
means "the code mentions this" is worth nothing on an installer.

Two failures during this work were the **test** being wrong, not the installer:
asserting that a display manager creates a symlink in `multi-user.target.wants/`
(it uses `Alias=display-manager.service` and creates none), and `test -e` on that
absolute alias, which resolves against the host root instead of the mount.

0.1.18 found two more the same way: the ESP label `CAMARELOL-EFI` is longer than
FAT's 11-character limit, so `mkfs.fat` refused every UEFI install at the format
phase; and the mount phase followed plan order, so the root mount hid the ESP
and `grub-install` rejected `/boot`. Both passed the entire suite, both died
only in a VM, and both now have tests that guard them — the label limit per
filesystem kind, and root-before-ESP mount order.

`[~]` is partial: part of the item ships and is verified, part does not exist.


## Stage 2 — Desktop

- [x] Desktop profiles: KDE, GNOME, Xfce, Cinnamon, LXQt, Hyprland, Sway —
      proved on a VirtualBox install (`pacstrap` 26 packages, `hyprland` on disk)
- [x] Wayland or X11, chosen with compatibility checks — Sway refuses X11;
      an explicit choice pulls that stack and only that stack
- [x] Display manager: SDDM, GDM, LightDM, greetd — installed, enabled, and
      greetd writes its own `/etc/greetd/config.toml`
- [~] Driver detection and firmware suggestions — **partial**: CPU microcode
      hints and a base set (`mesa`, `linux-firmware`) are chosen from the
      host, and a driver suggestion is now keyed off the parsed GPU model —
      `nvidia-open` on Turing and newer (an unreadable model never counts as
      Turing), plus `vulkan-radeon` + `libva-mesa-driver` on AMD and
      `vulkan-intel` + `intel-media-driver` on Intel; nothing for a virtual
      GPU. NVIDIA 590 removed the proprietary main packages, so Maxwell,
      Pascal and Volta get no driver and boot on `nouveau`. Unit-tested
      against real `lspci` lines, with a catalog test that every name it can
      produce is searchable. Not yet observed on a VM with a real GPU, which
      is what keeps this partial rather than done.
- [x] PipeWire, PulseAudio, or neither
- [x] Bluetooth, zram, swap choices
- [~] NetworkManager configuration — **partial**: the package is installed,
      the unit is enabled, and `/etc/NetworkManager/conf.d/00-camarelol.conf`
      pins `dhcp=internal` (unit-tested; not yet observed in a VM).
- [~] Stale firmware boot entries — **partial**: the bootloader phase now
      reads `efibootmgr -v` and deletes only `Boot####` entries whose
      `/File(…)` loader is missing from OUR ESP (GUID-matched);
      `FvFile`/`CdRom`/`Usb` devices and any other partition are never
      touched, and a broken listing skips the cleanup with a note instead
      of failing the install. This targets the `Boot0005` style of hang —
      firmware pointing at an `EFI/…` file that a previous attempt left
      behind. Unit-tested with a VBox-shaped listing; VM observation
      pending.

## Stage 3 — Advanced storage (unreleased)

Built in the working tree after `1.0.3`; not in any release yet.

- [~] LUKS2 root encryption — **partial**: the format/mount phases put a LUKS2
      container on the root partition, format and mount the mapper
      (`/dev/mapper/cryptroot`), write `/etc/crypttab`, add the `encrypt` and
      `keyboard` hooks to `HOOKS` before `filesystems`, write
      `cryptdevice=UUID=…:cryptroot root=/dev/mapper/cryptroot` into
      `GRUB_CMDLINE_LINUX`, install `cryptsetup` in the target, and add the
      passphrase to the preflight check (8 characters minimum, counted in
      characters not bytes). The passphrase never reaches the profile or a
      command line: it is kept in memory, travels to `cryptsetup` on stdin, and
      previews record only a placeholder. Layouts that v1 cannot encrypt
      honestly are refused rather than half-delivered — a separate `/home`,
      systemd-boot, and dual-boot beside another OS. Unit-tested end to end on
      the simulated runner; **no VM has booted an encrypted install yet**, so
      the initramfs prompt, the crypttab/hook interaction and GRUB's cmdline
      are unproven outside unit tests. `--luks-passphrase-file` supplies the
      passphrase for headless installs.
- [~] Btrfs subvolume layout — **partial**: a btrfs root is formatted once,
      mounted at the top level (`subvolid=5`), `@`, `@home` (only when `/home`
      is not a separate partition) and `@snapshots` are created, and the root is
      remounted from `@` with `/home` and `/.snapshots` mounted from their own
      subvolumes. Order and the `umount` between the two root mounts are
      unit-tested; `genfstab`'s output for those mounts is not yet observed in
      a VM, and no snapshot tooling is installed — this is the layout a
      snapshot tool needs, not the tool.
- [ ] Manual partition editor — no code.

## Stage 4 — Experience

- [x] Save and resume a configuration across runs — a saved profile can be
      loaded back into the wizard with `L` on the welcome screen; a corrupt
      file or a wrong `schema_version` is refused instead of silently replaced
      by defaults, and passwords are never restored (`#[serde(skip)]`).
- [x] Package search and selection — `/` filters the catalog, arrows move,
      Space toggles. AUR packages are only offered when AUR is enabled, and
      names come from the catalog, never invented.
- [x] AUR support, built as a normal user and never as root — manual
      `makepkg` plus `yay`/`paru`/`pikaur`/`trizen` detection. Three independent
      root guards; no helper is installed automatically.
- [x] Diagnostics report export — `--save-diagnostics <FILE>` writes a
      redacted report (host, survey, plan, phases, warnings, log paths).
- [x] More interface languages — Vietnamese, English and Simplified Chinese.
- [x] Better recovery guidance — when an install stops early the TUI lists the
      finished phases, the failed one and what is safe to retry, and says
      plainly that it cannot promise the machine will boot.

## Stage 5 — Shipping

- [x] Release pipeline: `scripts/make-release.sh` builds from the release tag,
      refuses a dirty working tree, checks the installed binary reports the tag
      it was built from (it caught a 0.1.10-shaped drift once), and gates the
      release on the VM acceptance run — only `SKIP_VM=1` skips the gate, and
      the skip is a deliberate, logged decision.
- [x] Automated VM acceptance harness — the guest posts each step's result over
      NAT to the host, so no Guest Additions are needed and a hung step cannot
      swallow the results of the steps before it. `camarelol-acceptance` ran
      7/7 on the 0.1.18 build.
- [x] GitHub release with checksummed artifacts — the binary, a source tarball,
      the Java catalog jar, a generated PKGBUILD that pins the tarball's sha256,
      and a `SHA256SUMS` covering all of them. Shipped as 0.1.18.
- [x] npm package `@wenker-ai/camarelo` — every pack downloads the binary from
      the GitHub release of exactly the version in `package.json`, verifies it
      against `SHA256SUMS`, requires `--version` to agree, runs the package
      tests, and only then packs; publish is a separate explicit step. 1.0.0
      was the first beta and 1.0.3 is the current one, all under the `beta`
      dist-tag; `latest` remains 0.1.22 until promotion.

## Stage 6 — Dual-boot beside an existing OS

- [x] Keep-everything planner (`plan_dual_boot`) — built from the live survey:
      a GPT+UEFI disk with partitions contributes every one of them as a
      `format=false` row, its ESP is reused at `/boot/efi`, and the new layout
      is placed by explicit start sector inside the largest aligned free gap,
      which the planner refuses to plan into when the request cannot fit. The
      survey had to carry partition START sectors for this to exist at all.
- [x] Append-only write path — `sgdisk --new=5:<start>:<end>` adds only the
      new partitions; there is no zap and no table rewrite, and the
      format/mount phases address kept partitions by their real number.
- [x] GRUB on the reused ESP with os-prober — `os-prober` joins the pacstrap
      list and `/etc/default/grub` gets `GRUB_DISABLE_OS_PROBER=false`, both
      only when the plan is a dual-boot one.
- [x] VM evidence — installs beside a fake Windows and boots —
      `scripts/vm-dualboot.sh` builds a Windows-shaped disk (ESP, MSR, 6 GiB
      C:, 1 GiB recovery — each carrying marker files), installs beside it
      and checks that every marker survived untouched, the root landed on a
      new partition 5, GRUB lives on the original ESP, `os-prober` is
      installed, and exactly 5 partitions exist afterwards (three green
      runs, 2026-10-22). Run 3 first proved the refusal gate instead: with
      firmware set to BIOS the planner refused with "dual-boot only plans a
      UEFI + GPT disk". A from-disk boot then proved the boot chain:
      NVRAM → `EFI/camareLOL/` on the reused ESP → Arch kernel → initramfs
      → login prompt.
- [x] Windows entry in the boot menu — the fake Windows now carries the pair
      os-prober's `efi/20microsoft` probe requires (`bootmgfw.efi` together
      with a `BCD`), and the generated `grub.cfg` gained
      `menuentry "Windows Boot Manager (on /dev/sda1)"` chainloading it —
      checked by the harness as `windows_entry ≥ 1` (run 7, 17 green
      checks, 2026-10-22). Two fixes made this honest rather than lucky:
      the `GRUB_DISABLE_OS_PROBER=false` flag is written by the bootloader
      phase itself (it used to land in system-config, which runs after
      `grub-mkconfig` — the flag came too late and even a real Windows
      would have been missed), and the BCD requirement was found by
      tracing `os-prober` inside the VM. What is NOT yet observed: the
      menu entry actually booting a real Windows — that stays deferred
      below. The reminder stands: **do not point camareLOL at the disk
      that holds your only Windows install.** First real-machine steps
      live in `docs/DUAL-BOOT-TEST.md`.
- [x] Real machine — installed beside EndeavourOS and booted — a live run
      of `scripts/dual-boot-auto.sh` (0.1.22) added a `camarelol-root`
      partition into the free gap of the author's real 119 GiB GPT disk:
      the existing 1 GiB ESP and the 69 GiB EndeavourOS root stayed
      `format=false` untouched, and the new ~49 GiB ext4 root landed in
      the gap left by a deleted first partition. First boot confirmed on
      that machine (24 Oct 2026): F11 selects `camareLOL` in the firmware
      boot menu, GRUB comes from `EFI/CAMARELOL/` on the reused ESP, and
      the new system reached a desktop login. Not yet observed on this
      leg: the return path — booting EndeavourOS again through the
      generated `endeavouros (EFI BootNext)` GRUB entry — and the
      `efibootmgr -o 0002,0000` default-order swap, both wait for the
      next reboot; the stale-entry cleanups (`EFI/archtest` plus its
      `Boot0001`, the boot-ISO `menuentry` on the old system's
      `grub.cfg`, the 1.6 GiB ISO under `/mnt/Storage`) are also pending.

## Future ideas (only an idea, no code)

**siroLOL — a no-USB Ventoy/Rufus replacement.** Sketched by the author on
2026-10-24, day 13 of development: instead of a USB stick, reserve a FAT32
partition on the disk, extract the source ISO into a directory there (FAT32's
4 GiB file-size cap rules out simply dropping the file in), and boot it the
way Ventoy and Rufus do, with a partition-level boot menu. Planned language
mix: Rust, C, C++, shell, HTML/CSS, Makefile. A bonus parallel: a FAT32
partition boots on far more firmware than an ext4 loopback does, precisely
because every UEFI can read FAT32. Nothing exists yet — not a design, not a
repo, not a line of code — and it changes nothing about camareLOL itself.
Two forms are imagined: a Windows-only GUI (easier to use, FAT32-only),
and the best-supported one — a Linux TUI/CLI with a list-style picker,
automatic .iso download, logo detection, and a deliberately manual part.

**tOrsiLOL — a preset-script distribution.** Also only a sketch: it would
install curated `.nix`, `.lua` and `.sh` script templates for Linux users
(auto-setup of Hyprland, hardening, theming, and similar presets), driven
by a GUI. Honest caveat recorded with the idea: `.nix` templates only have
their full meaning under NixOS or the Nix package manager, so either Nix
itself ships with the presets or the naming invites the misunderstanding
that any `.nix` applies anywhere. Sequenced after siroLOL: once there is a
standard install path, tOrsiLOL presets have something to stand on.
Nothing exists yet for this one either — no design, no repo, no code.

## Deliberately deferred

These are called out because their absence looks like an oversight and is
not.

**NTFS resize.** The acceptance criteria exist and the safety checks are
written down, but the feature is not built. Shrinking a partition that holds
somebody's only copy of Windows is a one-way operation with no undo, and it
belongs behind more testing than this project can currently claim.

**Dual-boot beside a real Windows.** The keep-everything side and the
Windows menu entry are done and ticked in Stage 6 above — VM-proven with a
fake Windows. What stays deferred is what only a real Windows can answer:
observing the menu entry actually boot Windows (the fake bootmgr is
chained but does nothing), and the whole shrink-from-inside-Windows
procedure. No release of camareLOL should go near a disk that holds the
user's only Windows until that leg closes.

**The rest of advanced storage.** LUKS2 root encryption and the btrfs
subvolume layout now have code, tests and their unproven legs written out in
Stage 3 above; what stays deferred is encrypting `/home` separately, encrypting
beside another operating system, and an encrypted root through systemd-boot —
each refused by the planner instead of approximated. A **manual partition
editor** still has no code at all, and the stage list once carried it as a
checkbox that said less than it appeared to.

In every case here the honest thing is to say so rather than to offer a
checkbox that does less than it appears to.

## Next

- [Architecture](architecture.md)
- [Testing](testing.md)