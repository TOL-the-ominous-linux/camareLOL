# Updates: when an update is refused

[← Documentation index](index.md)

Every refusal camareLOL can make, what causes it, and what to do about it.

## Reading an error

```console
error: [update_download_failed] could not download https://…
  (curl: (22) The requested URL returned error: 404)
  → try the next mirror, or pass --update-url to name one yourself
```

Three parts: a stable machine-readable code, a sentence saying what happened,
and an arrow with the next action. The codes below are taken from
`core/src/error.rs`; they are stable across releases, so a script can match on
them.

## Index and network

### `update_download_failed`

Every mirror was unreachable, or a proxy returned an error page.

```console
$ curl -sI https://camarelol.netlify.app/latest.json
$ camarelol --check-update --update-url https://example.org/latest.json
```

A mirror returning **200 with an HTML page** is not success. Some static hosts
fall back to `index.html` for unknown paths, so a 200 there means "not found"
wearing a success code. Check the content type, not just the status.

### `update_manifest_invalid`

The index is not a shape a client can act on: wrong `schema`, a missing field,
a non-HTTPS download URL, or a `sha256` that is not 64 hex characters.

This is a refusal rather than a best guess. An index camareLOL cannot fully
understand is an index it should not install from.

### `update_url_refused`

```console
error: [update_url_refused] this build was packaged without an update channel
  → updates come from the package manager that built it, or pass --update-url
```

The binary was built with `CAMARELOL_NO_UPDATE=1`. Correct by design — see
[building your own package](updates-packaging.md).

A plain-HTTP URL is refused for the same reason, unless it is loopback:
everything that is not `127.0.0.1` must be `https://`.

## Trust

### `update_signature_missing`

The index arrived with no signature. The mirror is serving `latest.json` without
`latest.json.sig`.

Serve both. If you control the mirror and genuinely cannot sign, read
[updates: signing](updates-signing.md) first — the override means accepting any
payload from that address.

### `update_signature_invalid`

The signature does not verify against the pinned key.

Usually a truncated upload, the wrong file, or a signature made with a different
key:

```console
$ gpg --verify latest.json.sig latest.json
```

The key that signed must be the key in `keys/camarelol.pub`. Verify before
re-uploading anything.

### `update_expired`

The index is past its `expires`. It is correctly signed and simply too old —
see [why an old index is refused](updates-rollback.md).

Either the mirror has not refreshed it, or this machine has been offline for
over a week. `--allow-expired-index` proceeds if you know why.

## The payload

### `update_checksum_mismatch`

The downloaded bytes do not match `sha256` in the index.

A truncated download, a proxy rewriting the body, or the release asset being
replaced after the index was signed.

**Do not** retry blindly. Compare what you have against what the release page
lists.

### `update_candidate_failed`

The new binary was downloaded and verified but would not run `--version`.

A missing library, or the wrong architecture. The old binary is left in place —
this check is why the swap happens only after the new copy has proved it can
start.

## The switch

### `update_not_privileged`

The binary is not writable by the current user.

```console
$ sudo camarelol --update
```

### An installation is running

`an installation is running`

Replacing the binary mid-install is the fastest way to leave a machine
half-installed, so the updater refuses. This holds even when the index is
correctly signed — a valid signature must not become a way past the safety rule.

### The update would not replace the running binary

The download landed somewhere other than the running executable. Pass
`--update-url` and copy explicitly, or install through a package manager.

### `update_rollback_failed`

`--rollback-update` found no `.prev` binary to restore. It only exists if an
update installed successfully in the first place.

## Things that are not errors

| message | meaning |
|---|---|
| `up to date` | the index matches or predates your version |
| `version 0.1.11 is available` | reported, nothing downloaded |
| exit code `10` | a newer version exists and nothing was done |

Exit code 10 exists so a script can nag without failing:

```console
camarelol --check-update || [ $? = 10 ] || exit 1
```

## When nothing works

Check each step by hand:

```console
$ curl -sS https://camarelol.netlify.app/latest.json | head -3
$ curl -sSI https://camarelol.netlify.app/latest.json.sig | head -1
$ camarelol --check-update --update-url https://… --update-key ./keys/camarelol.pub
```

If a mirror is wrong, say so — a dead mirror should be noticed by whoever runs
it, not by whoever next tries to update.

- <https://camarelol.netlify.app/status.html> — live mirror status
- <https://github.com/tamkogay9876-code/camareLOL/issues> — report it

## Next

- [Updates: signing](updates-signing.md)
- [Why an old index is refused](updates-rollback.md)
- [Building your own package](updates-packaging.md)