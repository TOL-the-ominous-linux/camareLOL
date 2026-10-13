# camareLOL — Testing guide

## 1. Unit tests

```console
$ cargo test --workspace          # Rust core + TUI
$ cd python && python3 -m unittest discover -s tests -v
$ cd java && ./build.sh           # compiles and runs CatalogTest
```

## 2. Acceptance scenarios (no real disks needed)

The specification's acceptance criteria run against synthetic device surveys:

```console
$ cd python && PYTHONPATH=. python3 -m camarelol_tools.testutil
PASS  empty-disk: self-test: OK
PASS  windows-disk: self-test: OK
PASS  partition-no-fs: self-test: OK
```

Scenarios: empty disk, Windows disk (ESP + recovery + MSR must be preserved),
partition without a filesystem, device identity change, invalid boot config.

## 3. VirtualBox full-system test

The real proof: boot an Arch ISO in a VM with an **empty** virtual disk and
run the installer in scripted mode.

```console
$ ./scripts/vm-test.sh create        # VM + 20 GiB empty disk + Arch ISO + data disk
$ ./scripts/vm-test.sh boot          # headless boot, drives the VM with keystrokes
$ ./scripts/vm-test.sh check         # screenshot + verify
$ ./scripts/vm-test.sh destroy
```

Inside the VM the script mounts the FAT data disk, scans the empty disk,
generates a profile from the survey and runs:

```console
# mount /dev/sdb /mnt
# /mnt/camarelol --scan > /tmp/survey.json
# python3 /mnt/scripts/vm-helper.py /tmp/survey.json /tmp/test-profile.toml
# /mnt/camarelol --scripted /tmp/test-profile.toml --allow-destructive --execute
```

### Result (verified on a BIOS VM, Arch Linux 7.2.7 live ISO)

```
preflight      completed  firmware=BIOS bootloader=GRUB fs=ext4 partitions=2
partition      completed  wrote GPT layout to /dev/sda
format         completed  filesystems created
mount          completed  target mounted
pacstrap       completed  installed 16 packages
fstab          completed  fstab generated
bootloader     completed  GRUB installed
system-config  completed  locale, keymap, timezone and hostname written
accounts       completed  0 user(s) configured
services       completed  services enabled
```

Verified afterwards inside the VM:

```console
# lsblk -f /dev/sda
sda1  vfat  LABEL=CAMARELOL                       (BIOS boot / ESP)
sda2  ext4  LABEL=camarelol-root   mounted on /mnt
# cat /mnt/etc/fstab
/dev/sdb   /boot vfat  defaults 0 1
/dev/sda2  /     ext4  defaults 0 2
# ls /mnt/boot
vmlinuz-linux  grub/  grub.cfg  i386-pc/
# ls /mnt/usr/bin/pacman
```

What the VM test proves:

* the binary runs in the Arch live environment (no missing libraries),
* the empty disk is detected and planned (GPT + partitions),
* the partition table is written and the filesystems are created,
* pacstrap installs the base system,
* `genfstab` output is valid, GRUB is installed with its BIOS modules,
* the target system has a kernel, a boot loader and a package manager.

### Bugs this test caught (all fixed)

1. The generated `sfdisk` script asked for more sectors than the disk had
   (alignment gaps and the GPT backup header were not accounted for).
2. The 1 MiB BIOS boot partition was silently dropped, because the generator
   stopped at a partition smaller than the alignment step.
3. The kernel was not asked to re-read the partition table before `mkfs`.
4. Mounting was gated under the pacstrap operation but confirmed at a
   different risk level, so the run stopped at `mount`.
5. `grub` was missing from the pacstrap list, so `grub-install` could not run
   inside the target.
6. `zram-config` is not a real unit; one missing unit aborted the whole
   services phase.

## 4. What is *not* tested here

* Real Windows dual-boot (needs a Windows license; the mock scenario covers the
  detection logic).
* NTFS resize (explicitly deferred to a later release — see ROADMAP.md).
* AUR builds (needs network + user interaction).
