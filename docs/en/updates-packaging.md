# Updates: building your own package

[← Documentation index](index.md)

If you package camareLOL — a PKGBUILD, an ebuild, an AppImage, your own mirror —
one decision matters more than the rest.

**A binary you ship should not offer users the upstream project's binaries.**

## Why

The upstream updater would offer a binary that your package manager never
built, never tested, and never signed with your distribution's process. A user
who installed from your repository would then be running someone else's build
of someone else's package. For an installer that partitions disks, that is not a
detail.

The alternative is to make your mirror the channel, which keeps a signature
check, a checksum, expiry, rollback and mirror fallback — and changes only the
address.

## Setting the channel at build time

Two variables, read **when the binary is built**:

```console
$ CAMARELOL_UPDATE_URL="https://mirror.example.org/camarelol/latest.json" cargo build --release
$ CAMARELOL_NO_UPDATE=1 cargo build --release      # or switch the updater off
```

| variable | effect |
|---|---|
| `CAMARELOL_UPDATE_URL` | the index your binary asks for |
| `CAMARELOL_NO_UPDATE` | no embedded channel at all; the updater refuses to run |

These are baked into the binary, not read from the environment at start-up. A
value taken from the environment could be set by whoever installed the package,
which is the wrong side to trust it from.

## Spelling

The variables are `CAMARELOL_UPDATE_URL` and `CAMARELOL_NO_UPDATE`. Both have
been written the wrong way round in documentation before — `CARAMELOL_` and
`CAMAMELOL_` — and a misspelled variable is ignored **silently**: the build
succeeds, the packager believes their mirror is configured, and every binary
they ship quietly updates from upstream instead.

The build now names the variable it actually reads when it sees a near miss:

```
warning: CARAMELOL_UPDATE_URL is set but this build reads CAMARELOL_UPDATE_URL.
The misspelled one is ignored, and the binary keeps the upstream update channel.
```

## Verifying it took effect

Do not assume. The binary knows what it was built with:

```console
$ camarelol --check-update
camareLOL 0.1.10 (tag v0.1.10, commit 8b765de)
index: https://mirror.example.org/camarelol/latest.json
binary: /usr/bin/camarelol
indexes to try: 2
answered by: https://mirror.example.org/camarelol/latest.json
trust: the index is signed by a pinned key
```

Check all three directions, not just one:

| build | `--check-update` should show |
|---|---|
| no variables | the project mirror |
| `CAMARELOL_UPDATE_URL` set | your mirror |
| `CAMARELOL_NO_UPDATE=1` | no address at all |

The third case is the one worth testing in both directions — set it, then unset
it and build again. Removing the variable has to return the default, not leave
the previous value baked in.

## Rebuilds

Cargo re-runs the build script when these change, so setting a variable before a
rebuild does take effect on that rebuild.

If you are unsure, build in a clean tree rather than deleting parts of `target/`
by hand:

```console
$ cargo clean && cargo build --release
```

Deleting `target/release/build/<pkg>` by hand breaks the fingerprint link, and
the build can then report itself as up to date when it is not.

## What your mirror must serve

```console
$ ./scripts/sign-release.sh 0.1.10
$ ls /var/www/camarelol/
latest.json  latest.json.sig
```

Both files, or the mirror is not a mirror. `latest.json.sig` is uploaded after
the release is created — the index cannot be signed before the release exists to
point at — so anything watching for a new release should wait for the signature
rather than fail on the first 404.

## Next

- [Updates: signing](updates-signing.md)
- [Why an old index is refused](updates-rollback.md)
- [When an update is refused](updates-troubleshooting.md)