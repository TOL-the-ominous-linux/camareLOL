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

---

## If you are on 0.1.10 or older

Those versions have an updater built in. Press `u` on the welcome screen and
it will check an address that no longer resolves, or silently report success
against a frozen index.

Nothing is at risk: it cannot install anything, because the server it asks is
gone. Download the current release and replace the binary as above. The `u` key
is not present in later versions.

---

## Checking what you have

```console
$ camarelol --version
camareLOL 0.1.11 (tag v0.1.11, commit abc1234)
```

Three parts, and they should agree:

| Part | Meaning |
|---|---|
| `0.1.11` | the release this binary was built from |
| `tag v0.1.11` | the git tag, which is what the release is named |
| `commit abc1234` | the exact commit |

If it says `[built from a dirty working tree]`, that binary was compiled from
uncommitted changes. It is not a release, whatever version it claims.

---

## Nothing else changes

Removing the updater removed only the code that replaced the binary. The
installer, the safety gate, the IPC protocol and the documentation are
unaffected. See [what is not built yet](roadmap.md) for where the project is
going.
