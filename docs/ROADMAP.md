# camareLOL roadmap

## Stage 1 — MVP (this release)

- [x] TUI framework with keyboard navigation and help
- [x] Environment checks (firmware, CPU, RAM, network)
- [x] Storage detection (lsblk/findmnt/blkid), risk assessment
- [x] Automatic layout for empty disks (GPT/MBR, ESP, BIOS boot, root, /home, swap)
- [x] Filesystem selection (ext4, btrfs, xfs)
- [x] Locale, keymap, timezone
- [x] Hostname, root, users, sudo policy (lock-out prevention)
- [x] Minimal Arch install (pacstrap + fstab + bootloader + services)
- [x] Logging and safe confirmation screens
- [x] Safety gate: locked / dry-run / execute with typed confirmation
- [x] Configuration save/load (schema-versioned, no passwords)
- [x] Python automation layer + Java catalog + versioned IPC

## Stage 2 — Desktop installer

- [ ] Desktop profiles (KDE, GNOME, Xfce, Cinnamon, LXQt, Hyprland, Sway)
- [ ] Wayland/X11 selection with compatibility checks
- [ ] Display manager selection (SDDM, GDM, LightDM, greetd)
- [ ] Driver detection and firmware suggestions
- [ ] PipeWire / PulseAudio / none
- [ ] Bluetooth, zram, swap choices
- [ ] NetworkManager configuration

## Stage 3 — Advanced storage

- [ ] Dual-boot: reuse an existing ESP, keep Windows/recovery/MSR partitions
- [ ] Manual partition editor (size, order, filesystem, mount point)
- [ ] LUKS encryption
- [ ] NTFS shrink (with the safety checks from the spec)
- [ ] Btrfs subvolume layouts

## Stage 4 — User experience

- [ ] Save/resume configuration across runs
- [ ] Advanced package selection with search
- [ ] AUR support (build as a normal user, never as root)
- [ ] Diagnostics report export
- [ ] More UI languages
- [ ] Better recovery guidance
