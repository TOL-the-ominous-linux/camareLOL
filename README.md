<p align="center">
  <img src="assets/camareLOL.png" alt="camareLOL" width="140">
</p>

# camareLOL

[![m0QpBjbzyd](https://yourimageshare.com/ib/m0QpBjbzyd.gif)](https://yourimageshare.com/ib/m0QpBjbzyd)

> A modern, safety-first TUI installer for Arch Linux — with a Vietnamese soul.

**English** · [Tiếng Việt](README.vi.md) · [中文](docs/zh/index.md)

## ⚠ BETA — 1.0.3 beta

camareLOL **1.0.3** is the current beta; **1.0.0** was the first. The roadmap
audit is complete: every item is either shipped with evidence, explicitly
marked partial, or deferred with a written reason. That is not the same as
"finished" — three desktop items are still partial, real-Windows dual-boot is
deferred, and this program erases disks.

Earlier releases proved why the evidence rule exists. Three package names
passed the entire unit suite and died only in a VirtualBox run:

- `greetd-tuigreeter` instead of `greetd-tuigreet` → **Hyprland and Sway could
  not be installed at all**
- `p7zip` instead of `7zip` → `p7zip` was dropped from the Arch repositories
- asking to install a package named `systemd-boot`, which does not exist →
  **every** systemd-boot install died at `pacstrap`

The installer now has guards for those failures, and the VM harness checks the
installed disk rather than the menu. Still:

1. Only run it on an **empty disk** — not one holding Windows or any data.
2. Run it from the **Arch live ISO**, never on the machine you are trying to
   fix.
3. Back up everything first.

`1.0.3` is a beta, not a promise of stability. The npm `latest` tag stays on
**0.1.22**; the beta is published under the `beta` dist-tag and as a GitHub
pre-release. Install it deliberately — see [Install 1.0.3 beta](#install-103-beta).

The full release notes, including evidence and known limitations, are in
[docs/releases/1.0.1.md](docs/releases/1.0.1.md). The first beta's notes remain
at [docs/releases/1.0.0.md](docs/releases/1.0.0.md).


camareLOL installs Arch Linux from the terminal without making you type dozens
of shell commands. It detects your hardware, proposes a safe layout, shows you
exactly what it is about to do, and only writes to a disk after you confirm it.

**It is not a shell wrapper.** The installer has its own pipeline: discovery →
plan → review → confirm → execute → verify, with a safety gate in front of every
destructive operation.

| | |
|---|---|
| Releases | <https://github.com/TOL-the-ominous-linux/camareLOL/releases> |
| Discussions | <https://github.com/TOL-the-ominous-linux/camareLOL/discussions> |
| Issues | <https://github.com/TOL-the-ominous-linux/camareLOL/issues> |

There is no website. The documentation is Markdown in `docs/`, which GitHub
renders in the repository itself — no server to run, nothing to break.

## Documentation

Written in three languages. Every page exists in all three, so each page's
table points sideways into the other two.

| Topic | English | Tiếng Việt | 中文 |
|---|---|---|---|
| Overview | [index](docs/en/index.md) | [VI](docs/vi/index.md) | [ZH](docs/zh/index.md) |
| 1.0.3 release notes | [releases/1.0.3](docs/releases/1.0.3.md) | — | — |
| 1.0.2 release notes | [releases/1.0.2](docs/releases/1.0.2.md) | — | — |
| 1.0.1 release notes | [releases/1.0.1](docs/releases/1.0.1.md) | — | — |
| 1.0.0 release notes | [releases/1.0.0](docs/releases/1.0.0.md) | — | — |
| The three components | [architecture](docs/en/architecture.md) | [VI](docs/vi/architecture.md) | [ZH](docs/zh/architecture.md) |
| The IPC protocol | [ipc-protocol](docs/en/ipc-protocol.md) | [VI](docs/vi/ipc-protocol.md) | [ZH](docs/zh/ipc-protocol.md) |
| What gets tested | [testing](docs/en/testing.md) | [VI](docs/vi/testing.md) | [ZH](docs/zh/testing.md) |
| Running it yourself | [vm-testing](docs/en/vm-testing.md) | [VI](docs/vi/vm-testing.md) | [ZH](docs/zh/vm-testing.md) |
| Updating, and why there is none | [update](docs/en/update.md) | [VI](docs/vi/update.md) | [ZH](docs/zh/update.md) |
| What is not built yet | [roadmap](docs/en/roadmap.md) | [VI](docs/vi/roadmap.md) | [ZH](docs/zh/roadmap.md) |

Start with [the overview](docs/en/index.md).

## What changed in 1.0.3

**One mislabelled line, fixed.** The 1.0.2 run-in-QEMU evidence honestly noted
a virtio disk showing `kind: "SataHdd"` while `transport: "virtio"` sat on the
same line. The kind classifier consulted the `rotational` flag before the
virtio name prefixes — some hypervisors set that flag on sparse images.
1.0.3 reorders the checks: a virtio disk is correctly labelled
`Virtual Disk`, with regression tests pinning both the virtio and the
genuine-SATA cases. Cosmetic fix only; planning never depended on the label.

The full notes are in [docs/releases/1.0.3.md](docs/releases/1.0.3.md).

## What changed in 1.0.2

**An honesty fix in `--self-test`.** On a machine camareLOL does not support
(say, Ubuntu), the 1.0.1 self-test reported FAIL even though every machinery
check had passed — what failed was the host gate, by design. 1.0.2 reports
`SKIP (host unsupported)` with **exit 2** for that case: `0` = PASS, `2` =
environment out of scope, `1` = a machinery check failed (report that).
Exit codes are now documented on the quick-start pages, and the release
notes carry first-hand output from a full Arch-in-QEMU run of the 1.0.1
assets — including a 12-phase simulated install and a clean TUI session.

The full notes are in [docs/releases/1.0.2.md](docs/releases/1.0.2.md).

## What changed in 1.0.1

**A discovery and self-test fix.** `lsblk` labels zram, ram, nbd, md and loop
devices as type `disk`; camareLOL listed them on the Storage screen. On a host
with zram, `--self-test` picked zram as the empty disk, failed the size check
and reported FAIL. A zram large enough to pass the planner's minimum would have
been a target for an install into volatile memory.

1.0.1 filters those virtual block devices at discovery, and `--self-test` now
skips the simulated install when there is no empty disk instead of running it
against the installed system. No planning, partitioning, `pacstrap`,
bootloader, desktop, safety-gate, IPC or config-schema behaviour changed.

The full notes are in [docs/releases/1.0.1.md](docs/releases/1.0.1.md).

## What changed in 1.0.0

**The first beta.** 1.0.0 is the same installer code as 0.1.22, re-versioned
and re-documented after a complete roadmap audit. No discovery, planning,
partitioning, `pacstrap`, bootloader, desktop, safety-gate, IPC or config-schema
behaviour changed.

What did change:

- The Cargo workspace, `Cargo.lock`, npm package, PKGBUILD template and
  dual-boot helper now agree on **1.0.0**.
- The GitHub release is a **pre-release**; npm publishes 1.0.0 under the
  **`beta`** tag. `npm install -g @wenker-ai/camarelo` still installs 0.1.22.
- The docs were corrected: stale alpha wording, the removed `u` update key,
  and the old "layout UI comes in Stage 3" claim are gone.
- Release tooling now reads notes from `docs/releases/<version>.md`, supports
  a pre-release flag, supports `NPM_TAG=beta`, and accepts
  `CAMARELOL_CHANNEL=beta`.

The full, item-by-item notes — including the three partial Stage 2 items and
the deferred real-Windows work — are in
[docs/releases/1.0.0.md](docs/releases/1.0.0.md).

If you are on 0.1.10 or older, you still have the removed updater in your
binary. It asks an address that no longer resolves and cannot install
anything. Download 1.0.3 and replace the binary by hand — see
[Updating](docs/en/update.md).

## Install 1.0.3 beta

The beta is opt-in. The default one-liner resolves the newest **stable**
release, which is still 0.1.22. To install the beta, pin the tag or ask for the
beta channel:

```console
$ curl -fsSL https://raw.githubusercontent.com/TOL-the-ominous-linux/camareLOL/main/scripts/camarelol.sh \
    | CAMARELOL_VERSION=v1.0.3 bash
```

```console
$ curl -fsSL https://raw.githubusercontent.com/TOL-the-ominous-linux/camareLOL/main/scripts/camarelol.sh \
    | CAMARELOL_CHANNEL=beta bash
```

Via npm:

```console
$ npm install -g @wenker-ai/camarelo@beta
```

## Install

### One line (the bridge)

The fastest way in, and the one that cannot install the wrong thing:

```console
$ curl -fsSL https://raw.githubusercontent.com/TOL-the-ominous-linux/camareLOL/main/scripts/camarelol.sh | bash
```

It resolves the newest release, **verifies it three ways** — HTTP status, ELF
magic, SHA-256 — and then hands control back to you. That third check is not
decoration:

```console
$ curl -sSLO https://github.com/.../camarelol    # no -f, no -L
$ chmod +x camarelol
$ ./camarelol --self-test
./camarelol: line 1: Not: command not found
```

Without `-f`, curl saves a proxy's 404 page to a file named `camarelol` and
still exits 0. `chmod +x` succeeds on it. Bash then tries to run the HTML as a
script. An installer that erases disks has no business telling you "done" when
what it downloaded is an error page.

### From a release

Download the binary from
[Releases](https://github.com/TOL-the-ominous-linux/camareLOL/releases) and run it
from the Arch live ISO:

```console
$ chmod +x camarelol
$ ./camarelol --self-test      # non-destructive checks; safe on a real machine
$ sudo ./camarelol             # the TUI
```

Or let the helper fetch it for you:

```console
$ git clone https://github.com/TOL-the-ominous-linux/camareLOL
$ cd camareLOL && sudo ./install.sh --binary
$ CAMARELOL_VERSION=v1.0.3 sudo -E ./install.sh --binary   # beta tag
```

`install.sh --binary` downloads the release `SHA256SUMS` and refuses a binary
that does not match it.

### Via npm

On a running Arch system, npm is one more way in:

```console
$ sudo pacman -S --needed nodejs npm curl
$ npm install -g @wenker-ai/camarelo          # stable latest: 0.1.22
$ npm install -g @wenker-ai/camarelo@beta     # 1.0.3 beta
```

On the **archiso** (live ISO), the pacman database is empty — use
`sudo pacman -Sy --needed nodejs npm curl` after connecting to the network,
and see [Updating](docs/en/update.md).

The package carries the release binary plus a small Node wrapper that runs
before it. If the bundled build is older than the newest GitHub release, the
wrapper can fetch that release (verified against the release `SHA256SUMS`) and
replace its own copy. The binary never replaces itself — what that does and
does not protect against is written in [Updating](docs/en/update.md).

### Updating

camareLOL does not update itself. There is no updater, no index URL, and no
code path in the binary that replaces it.

That is a decision, not an unfinished feature. This program erases disks; a
component whose job is to replace itself is the shortest path from a compromised
server to erased data. There is no `pacman` package and no AUR package. Re-run
the one-liner; it picks the newest release and verifies it:

```console
$ curl -fsSL https://raw.githubusercontent.com/TOL-the-ominous-linux/camareLOL/main/scripts/camarelol.sh | bash
```

Or download the release you want from
[Releases](https://github.com/TOL-the-ominous-linux/camareLOL/releases) and replace
the binary by hand.

### From source

```console
$ ./install.sh                # builds and installs to /usr/local/bin
$ ./build.sh build            # compile everything, no install
$ ./build.sh test             # unit tests + acceptance scenarios
$ ./build.sh lint             # clippy + rustfmt
```

### From the tarball (`makepkg`)

Each release ships a `PKGBUILD` pinned to that release's tarball:

```console
$ VERSION=1.0.3
$ curl -fLO "https://github.com/TOL-the-ominous-linux/camareLOL/releases/download/v$VERSION/camarelol-$VERSION.tar.gz"
$ curl -fLO "https://github.com/TOL-the-ominous-linux/camareLOL/releases/download/v$VERSION/PKGBUILD"
$ tar xf "camarelol-$VERSION.tar.gz"
$ cp PKGBUILD "camarelol-$VERSION/"
$ cd "camarelol-$VERSION"
$ makepkg -si
```

The `PKGBUILD` attached to the release carries the tarball's own SHA-256; the
copy inside the tarball cannot, because it was sealed before the checksum was
known. Use the attached file.

`releases/latest/download/` takes **the exact file name**, so it has to be built
from the tag name; there is no way to use that path without knowing the version
in advance. That is why the 1.0.3 beta uses explicit `v1.0.3` URLs.

The full VirtualBox acceptance test is documented in
[docs/en/testing.md](docs/en/testing.md), and a hands-on guide for running it
yourself is in [docs/en/vm-testing.md](docs/en/vm-testing.md).

## What it does today

- Detects disks, partitions, filesystems, EFI partitions, Windows, recovery and
  Microsoft-reserved partitions, live-system mounts.
- Plans GPT/MBR layouts: ESP, BIOS boot, `/`, `/home`, swap — with filesystem
  choices (ext4, btrfs, xfs) and firmware-aware validation.
- Detects Windows and refuses to touch it. Keep-everything dual-boot is
  implemented: existing partitions stay `format=false`, the ESP is reused, and
  new partitions are appended inside a free gap; a real-machine install and
  first boot were confirmed on 24 October 2026.
- Installs a minimal Arch system: pacstrap, fstab, bootloader (GRUB/systemd-boot),
  locale, keymap, timezone, hostname, users, sudo policy, services, initramfs.
- Safety gate: locked by default; `--allow-destructive` + typed confirmation
  phrases (`ERASE`, `ERASE AND WIPE`) arm it; `--execute` is required to write.
- `--preview`: simulates a complete installation — all twelve phases, every real
  command printed, **nothing executed and nothing written**. It ignores
  `--allow-destructive` and `--execute`, and a red banner stays on screen for
  the whole run.
- Configuration profiles: save/load, schema-versioned, **never containing
  passwords**.
- Logs everything with secret redaction; diagnostics report for bug reports.

## What it deliberately does not do yet

- Format a disk just because you selected it in a menu.
- Install AUR packages without an explicit opt-in and a normal-user build.
- Resize NTFS (the acceptance criteria for it are documented but the feature is
  deferred — see [docs/en/roadmap.md](docs/en/roadmap.md)).
- Boot a real Windows install from the generated menu entry, or shrink Windows
  from inside Windows. The keep-everything side and the menu entry are
  VM-proven; the real-Windows leg is deferred.
- Suggest a GPU driver from the parsed model. The model is read and shown; a
  model-keyed driver choice is still partial.
- Manual partitioning, LUKS and Btrfs subvolume layouts — no code.

## Architecture

Three components, one protocol:

| component | role |
| --- | --- |
| **Rust core** (`core/`, `tui/`) | authoritative: discovery, planning, safety, install |
| **Python** (`python/`) | validation, recommendations, diagnostics, tests |
| **Java** (`java/`) | package catalog, desktop profiles, plugin interface, JSON-RPC |

See [docs/en/architecture.md](docs/en/architecture.md) and
[docs/en/ipc-protocol.md](docs/en/ipc-protocol.md).

## Installer flow

```
Welcome → Language → Keyboard → Timezone → Storage → Disk mode →
Layout → Filesystem → Accounts → Bootloader → Desktop → Services →
Packages → Review (type the confirmation phrase) → Progress → Result
```

Keyboard only: `↑↓` move, `Enter` select, `Space` toggle, `Tab` next field,
`Esc` back, `F1` help, `F5` re-scan, `F10` review, `Ctrl-C` quit.

## Testing

```bash
./build.sh test                                # everything (Rust + Python + Java)
cd python && python3 -m unittest discover -s tests -v
cd java && ./build.sh
python scripts/check_lang_table.py              # doc tables point at the right language
python scripts/check_pkgbuild.py                # PKGBUILD matches the newest tag
python scripts/check_secrets.py                 # no credential in the tree
python scripts/check_workflows.py               # workflow YAML is valid to GitHub
cargo clippy --workspace --all-targets -- -D warnings
```

A full install is verified in a VirtualBox VM — see
[docs/en/testing.md](docs/en/testing.md).

## Part of The Ominous Linux

camareLOL is the first project of **The Ominous Linux (TOL)** — a name for Linux
tooling that states the risk and shows the evidence. The planned **iLOL kernel**
line has its own repository:
<https://github.com/TOL-the-ominous-linux/ilol-kernel>.

- [What TOL is](tol/README.md) — landing page:
  <https://github.com/TOL-the-ominous-linux/.github>
- [Brand and logo usage](tol/BRAND.md) — the mark is inspired by the band TOOL
  and carries that attribution line
- [iLOL kernel](tol/LOL-KERNEL.md) — planned, not shipped;
  [kernel repo](https://github.com/TOL-the-ominous-linux/ilol-kernel)

## I USE ARCH BTW

<p align="center">
  <img src="assets/arch.png" alt="Arch Linux" width="110">
</p>

Installing Arch is a choice, not a chore. camareLOL exists so that the choice
stays yours: you can read the whole plan before anything is written, and you
can walk away at any point before the confirmation phrase.

## License

GPL-3.0-or-later. See the headers in each file.

## Warning

Installing an operating system and partitioning disks **can destroy data**.
camareLOL asks before every irreversible step, re-identifies the disk before
writing, and never claims success it did not verify. Still: back up anything you
care about before you install anything.