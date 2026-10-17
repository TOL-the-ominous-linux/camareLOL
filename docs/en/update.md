# Updating camareLOL

[← Documentation index](index.md)

camareLOL does not update itself.

---

## There is no updater

No `--update` flag. No index URL. No code path in the binary that replaces
itself.

This is a decision, not an unfinished feature. camareLOL erases disks. A
component whose job is to replace itself is the shortest path from a compromised
server to erased data — the signature check protects you from a tampered
*download*, and nothing else. The trust anchor is a server that is, by
construction, reachable by whoever attacked it.

The mechanism underneath it was also a liability. Before it was removed, every
release shipped with a signing key, an update index, a mirror list and two
mirror hosts. That chain broke four times in one day. The failure mode was
always the same: one host stopped answering, another kept serving a frozen copy
of the index, and every camareLOL in the field reported "you are up to date".

**A mirror that lies about freshness is worse than no mirror at all.**

---

## How to update instead

### From a repository or the AUR

The normal way, like every other Arch package:

```console
$ sudo pacman -S camarelol
```

### By hand

Download the release you want from
[Releases](https://github.com/tamkogay9876-code/camareLOL/releases) and replace
the binary:

```console
$ sudo pacman -S wget                 # if you do not have it
$ sudo wget -O /usr/local/bin/camarelol \
    https://github.com/tamkogay9876-code/camareLOL/releases/latest/download/camarelol
$ sudo chmod +x /usr/local/bin/camarelol
$ camarelol --version                  # check what you now have
```

There is no signature to verify, because there is nothing to verify against.
`--version` tells you which release you are running, and that is the whole of
the trust story.

### Via npm

Requires `node` and `npm`:

```console
$ sudo pacman -S --needed nodejs npm curl
$ npm install -g @wenker-ai/camarelo
```

On **archiso** (the live ISO), one extra step: the pacman databases are empty,
so plain `pacman -S` fails with `database file for 'core' does not exist`.
Download them and install in one go — with a network already up (Wi-Fi lives
outside this flow; pacman needs it to fetch the database):

```console
$ sudo pacman -Sy --needed nodejs npm curl
```

The npm wrapper is not inside the binary: it is Node code that runs *before*
the binary starts, and the binary still has no code path that replaces itself.
When the bundled build is older than the latest GitHub release, the wrapper
asks once — answer `y` and it downloads that build, checks it against
`SHA256SUMS`, replaces the bundled binary and hands over; answer `n` and it
goes straight to the installer.

The trust story is the same as downloading by hand: the checksum file lives in
the same place as the binary, so it catches a download corrupted in transit —
not a compromised server.

---

## If you are on 0.1.10 or older

Those versions have an updater built in. Press `u` on the welcome screen and
it will check an address that no longer resolves, or silently report success
against a frozen index.

Nothing is at risk: it cannot install anything, because the server it asks is
gone. Download the current release and replace the binary as above. The `u` key
is not present in later versions.

---

## If your system was installed with 0.1.12 and zram was selected

0.1.12 enabled the right unit (`systemd-zram-setup@zram0.service`) and
installed `zram-generator` — and that still is not a working zram swap.
`zram-generator` reads `/etc/systemd/zram-generator.conf` to know which
devices to create; with the file absent it runs and does nothing, and the
system boots with no zram at all. 0.1.13 closes that gap: during install the
installer writes the config file according to the `zram_size_mb` choice.

A 0.1.12-installed system already has the unit and the package; only the
config file is missing. The lightest fix is adding the file by hand — no
reinstall needed:

```console
$ sudo tee /etc/systemd/zram-generator.conf >/dev/null <<'EOF'
[zram0]
zram-size = min(ram / 2, 4096)
EOF
$ sudo systemctl start systemd-zram-setup@zram0
$ zramctl
```

`min(ram / 2, 4096)` is the installer's default: half of RAM, capped at 4
GiB. For a different size write a plain MiB number, e.g. `zram-size = 2048`.
Afterwards `zramctl` should list `zram0`; if it does not, reboot and check
again.

---

## Checking what you have

```console
$ camarelol --version
camareLOL 0.1.12 (tag v0.1.12, commit abc1234)
```

Three parts, and they should agree:

| Part | Meaning |
|---|---|
| `0.1.12` | the release this binary was built from |
| `tag v0.1.12` | the git tag, which is what the release is named |
| `commit abc1234` | the exact commit |

If it says `[built from a dirty working tree]`, that binary was compiled from
uncommitted changes. It is not a release, whatever version it claims.

---

## Nothing else changes

Removing the updater removed only the code that replaced the binary. The
installer, the safety gate, the IPC protocol and the documentation are
unaffected. See [what is not built yet](roadmap.md) for where the project is
going.
