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
$ ./scripts/make-data-iso.sh target/release/camarelol   # binary on a data ISO
$ ./scripts/vm-test.sh create                            # VM + empty disk
$ ./scripts/vm-test.sh boot                              # headless boot
# inside the VM:
#   mount /dev/sr1 /mnt
#   /mnt/camarelol --scripted /mnt/test-profile.toml --allow-destructive --execute
$ ./scripts/vm-test.sh check                             # verify the result
$ ./scripts/vm-test.sh destroy
```

What the VM test proves:

* the binary runs in the Arch live environment (no missing libraries),
* the empty disk is detected and planned (GPT + ESP + root),
* the partition table is written and the filesystems are created,
* pacstrap installs the base system,
* the bootloader is installed,
* the post-install checks pass.

## 4. What is *not* tested here

* Real Windows dual-boot (needs a Windows license; the mock scenario covers the
  detection logic).
* NTFS resize (explicitly deferred to a later release — see ROADMAP.md).
* AUR builds (needs network + user interaction).
