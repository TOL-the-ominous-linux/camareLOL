# Roadmap

[← Documentation index](index.md)

What is built, what is not, and — where it is not — why.

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
      hints and a base set (`mesa`, `linux-firmware`) are chosen from the host.
      GPU model detection is not implemented.
- [x] PipeWire, PulseAudio, or neither
- [x] Bluetooth, zram, swap choices
- [~] NetworkManager configuration — **partial**: the package is installed and
      the unit is enabled. No `/etc/NetworkManager` file is written.

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
      tests, and only then packs; publish is a separate explicit step. 0.1.18
      is the published latest.

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
      installed, and exactly 5 partitions exist afterwards (two full green
      runs, 2026-10-22). The third run first proved the refusal gate instead:
      with firmware set to BIOS the planner refused with
      "dual-boot only plans a UEFI + GPT disk". A from-disk boot then proved
      the boot chain: NVRAM → `EFI/camareLOL/` on the reused ESP → Arch
      kernel → initramfs → login prompt.
- [ ] Windows entry in the boot menu — os-prober finds a Windows through its
      real boot manager; the fake Windows carries none, so the menu entry has
      not been observed yet, and the from-inside-Windows shrink procedure is
      not exercised by a VM. The reminder stands: **do not point camareLOL at
      the disk that holds your only Windows install.** First real-machine
      steps live in `docs/DUAL-BOOT-TEST.md`.

## Deliberately deferred

These are called out because their absence looks like an oversight and is
not.

**NTFS resize.** The acceptance criteria exist and the safety checks are
written down, but the feature is not built. Shrinking a partition that holds
somebody's only copy of Windows is a one-way operation with no undo, and it
belongs behind more testing than this project can currently claim.

**Dual-boot beside a real Windows.** The keep-everything side is done and
ticked in Stage 6 above — VM-proven with a fake Windows and a from-disk boot.
What stays deferred is the last leg: observing the Windows entry on the boot
menu (needs a Windows that actually boots in a VM), and the whole
shrink-from-inside-Windows procedure. No release of camareLOL should go near
a disk that holds the user's only Windows until that leg closes.

**Everything else from advanced storage** — a manual partition editor, LUKS
encryption, Btrfs subvolume layouts — has no code at all. The stage list once
carried them as five unticked checkboxes; that said less than it appeared to,
so they live here instead.

In every case here the honest thing is to say so rather than to offer a
checkbox that does less than it appears to.

## Next

- [Architecture](architecture.md)
- [Testing](testing.md)