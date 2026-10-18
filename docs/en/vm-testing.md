# Running it yourself

[← Documentation index](index.md)

Two ways: **preview**, which writes nothing, and **real install**, which writes
to a virtual disk. Both need VirtualBox.

---

## Way 1 — Preview, writes nothing

Use this first. It exercises the interface and disk detection against a real
machine without touching anything.

```console
$ cd /path/to/camareLOL
$ bash scripts/vm-release-test.sh create    # VM: Arch ISO + two empty disks
$ bash scripts/vm-release-test.sh boot      # headless; downloads the binary, runs self-test
$ bash scripts/vm-release-test.sh verify    # screenshot the result
```

Inside the VM:

```console
$ camarelol --version
$ camarelol --scan          # JSON: model, serial, partitions, risk level
$ camarelol --self-test     # non-destructive checks
$ camarelol                 # open the TUI; typing nothing writes nothing
```

`--self-test` is safe to run on a real machine. So is `--scan`. The TUI only
writes when you type the confirmation phrase.

Screenshots land in `/tmp/opencode/vm-shots/`.

---

## Way 2 — Real install to a virtual disk

> **This script erases data on the virtual disk.** It is for VMs only. Do not
> point it at a real machine.

```console
$ cd /path/to/camareLOL
$ bash scripts/vm-test.sh create    # VM + empty 20 GiB disk + FAT disk holding the binary
$ bash scripts/vm-test.sh boot      # boot, type the install commands automatically
$ bash scripts/vm-test.sh check     # screenshot the result
$ bash scripts/vm-test.sh destroy   # delete the VM
```

Expected: all phases complete, a valid `/mnt/etc/fstab`, and `vmlinuz-linux`
plus `grub.cfg` present.

---

## Creating a VM by hand

If you want to see the actual screen.

```console
$ VBoxManage createvm --name lol --ostype ArchLinux_64 --register
$ VBoxManage modifyvm lol --memory 4096 --cpus 4 --boot1 dvd --boot2 disk
$ VBoxManage storagectl lol --name SATA --add sata --controller IntelAhci
$ VBoxManage createmedium disk --filename ~/lol.vdi --size 20480 --format VDI
$ VBoxManage storageattach lol --storagectl SATA --port 0 --device 0 \
    --type hdd --medium ~/lol.vdi
$ VBoxManage storagectl lol --name IDE --add ide
$ VBoxManage storageattach lol --storagectl IDE --port 0 --device 0 \
    --type dvddrive --medium "$PWD/.test/archlinux-x86_64.iso"
$ VBoxManage startvm lol            # NOT headless — you want to see it
```

Inside the live ISO:

```console
# mount -o remount,rw /          # a live ISO usually mounts read-only
# pacman -Sy --needed curl
# curl -fLO https://github.com/tamkogay9876-code/camareLOL/releases/latest/download/camarelol
# chmod +x camarelol
# ./camarelol --self-test
# sudo ./camarelol --allow-destructive    # add --execute to actually write
```

Everything camareLOL needs is already in the live ISO: `sfdisk`, `sgdisk`,
`mkfs.*`, `pacstrap`, `genfstab`, `arch-chroot`. Nothing extra to install.

## Without a terminal

Running it over a pipe, from `cron`, or in CI fails with a clear message rather
than a panic — the TUI has nowhere to draw:

```
camareLOL needs a terminal to draw its interface, and there is none here.

For unattended runs there is no interface to draw, so use the scripted path:
    camarelol --scripted <config.toml>
and add --allow-destructive --execute to actually write to a disk.
```

---

## What to check

| do this | expect |
| --- | --- |
| open the TUI, move with `↑↓` | model, serial and risk level shown per disk |
| select a disk, press `F10` | lists the destructive operations and asks for `ERASE` |
| type the phrase wrong | refused, and **nothing is written** |
| press `F1` twice | the help overlay opens and closes |
| run a real install | every phase completes, valid `fstab`, GRUB installed |
| pull the network cable, then continue | reports a network error, never a false success |

The third row is the one that matters. Everything else is a convenience.

---

## Cleaning up

```console
$ bash scripts/vm-test.sh destroy
$ bash scripts/vm-release-test.sh destroy
$ rm -rf ~/VirtualBox\ VMs/camarelol-*
```

Virtual disks and the ISO live in the project's `.test/` directory. Deleting the
VM does not affect the host.

## Next

- [Testing](testing.md)
- [What is not built yet](roadmap.md)