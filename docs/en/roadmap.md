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

## Stage 3 — Advanced storage

- [ ] Dual-boot: reuse an existing ESP, preserve Windows, recovery and MSR
      partitions
- [ ] Manual partition editor: size, order, filesystem, mount point
- [ ] LUKS encryption
- [ ] NTFS shrink, with the safety checks already written in the specification
- [ ] Btrfs subvolume layouts

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

## Deliberately deferred

Two items are called out because their absence looks like an oversight and is
not.

**NTFS resize.** The acceptance criteria exist and the safety checks are
written down, but the feature is not built. Shrinking a partition that holds
somebody's only copy of Windows is a one-way operation with no undo, and it
belongs behind more testing than this project can currently claim.

**Reusing an existing ESP for dual-boot.** The detection and preservation rules
are implemented and the planning code knows how to leave Windows partitions
alone. What is missing is the layout interface, and shipping half of it would
mean showing someone a plan they cannot safely confirm.

In both cases the honest thing is to say so rather than to offer a checkbox
that does less than it appears to.

## Next

- [Architecture](architecture.md)
- [Testing](testing.md)