# Testing

[← Documentation index](index.md)

Four layers, each answering a different question. The one that matters most is
the last: whether it works on a machine that actually has to boot.

## 1. Unit tests

```console
$ cargo test --workspace                        # Rust core + TUI
$ cd python && python3 -m unittest discover -s tests -v
$ cd java && ./build.sh                         # compiles, runs CatalogTest, builds the jar
$ python site/test_markdown.py                  # site renderer
$ python site/test_render_site.py               # site build, offline
$ python scripts/check_pkgbuild.py              # PKGBUILD matches the newest tag
```

Fast, no disks, no network.

## 2. Acceptance scenarios

No real disks needed. The specification's acceptance criteria run against
synthetic device surveys:

```console
$ cd python && PYTHONPATH=. python3 -m camarelol_tools.testutil
PASS  empty-disk: self-test: OK
PASS  windows-disk: self-test: OK
PASS  partition-no-fs: self-test: OK
```

| scenario | what it must prove |
| --- | --- |
| empty disk | a full layout can be planned and applied |
| Windows disk | ESP, recovery and Microsoft-reserved partitions are preserved, not reformatted |
| partition without a filesystem | detected as unformatted rather than assumed usable |
| device identity change | the run aborts instead of writing to a different disk |
| invalid boot config | refused at plan time, not discovered halfway through |

These run against the built binary with `--mock`, so they test the real code
path rather than a mock of it.

## 3. Full-system test in VirtualBox

The real proof. Boot an Arch ISO in a VM with an **empty** virtual disk and run
the installer in scripted mode.

```console
$ ./scripts/vm-test.sh create      # VM + 20 GiB empty disk + Arch ISO + data disk
$ ./scripts/vm-test.sh boot        # headless boot, drives the VM with keystrokes
$ ./scripts/vm-test.sh check       # screenshot and verify
$ ./scripts/vm-test.sh destroy
```

Inside the VM:

```console
# mount /dev/sdb /mnt
# /mnt/camarelol --scan > /tmp/survey.json
# python3 /mnt/scripts/vm-helper.py /tmp/survey.json /tmp/test-profile.toml
# /mnt/camarelol --scripted /tmp/test-profile.toml --allow-destructive --execute
```

### What success looks like

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
sda1  vfat  LABEL=CAMARELOL                       (ESP)
sda2  ext4  LABEL=camarelol-root   mounted on /mnt
# cat /mnt/etc/fstab
/dev/sda2  /     ext4  defaults 0 2
# ls /mnt/boot
vmlinuz-linux  grub/  grub.cfg  i386-pc/
# ls /mnt/usr/bin/pacman
```

### What that proves

- the binary runs in the Arch live environment, with no missing libraries;
- the empty disk is detected and a GPT layout planned;
- the partition table is written and filesystems created;
- pacstrap installs a base system;
- `genfstab` output is valid and GRUB is installed with its BIOS modules;
- the target has a kernel, a bootloader and a package manager.

A green unit suite says none of that.

### Bugs this caught

All of these passed every unit test.

1. The generated `sfdisk` script asked for more sectors than the disk had —
   alignment gaps and the GPT backup header were not accounted for.
2. The 1 MiB BIOS boot partition was silently dropped, because the generator
   stopped at a partition smaller than the alignment step.
3. The kernel was never asked to re-read the partition table before `mkfs`.
4. Mounting was gated under the pacstrap operation but confirmed at a different
   risk level, so runs stopped at `mount`.
5. `grub` was missing from the pacstrap list, so `grub-install` could not run
   inside the target.
6. `zram-config` is not a real unit; one missing unit aborted the whole
   services phase.

## 4. Packaging

CI builds the release tarball and runs `makepkg` on it inside an Arch
container. A `PKGBUILD` that cannot build fails the build rather than waiting
for someone to report it.

This exists because the recipe once said `pkgver=0.1.3` while the release was
`0.1.10`, with a `source` pointing at a tarball that was never published.

## What is not tested

| not covered | why |
| --- | --- |
| Real Windows dual-boot | needs a Windows licence; the mock scenario covers the detection logic |
| NTFS resize | explicitly deferred to a later release |
| AUR builds | needs network and user interaction |

## Running it yourself

[Running it yourself](vm-testing.md) has the hands-on guide, including a
preview mode that writes nothing.

## Next

- [Architecture](architecture.md)
- [Running it yourself](vm-testing.md)