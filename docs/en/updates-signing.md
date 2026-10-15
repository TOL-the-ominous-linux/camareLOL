# Updates: signing

[← Documentation index](index.md)

A checksum proves a file arrived intact. It does not prove the file came from
anyone you trust — if the server hands you both the payload and the matching
hash, they match, and that agreement means nothing.

So the update index is signed separately, and the key is **compiled into the
binary**.

## The index

`latest.json` is what a client asks for:

```json
{
  "schema": 1,
  "version": "0.1.10",
  "url": "https://github.com/tamkogay9876-code/camareLOL/releases/download/v0.1.10/camarelol",
  "sha256": "fbfe9d8e84fd62b733270932a35113afeb14fa27b6749433d5ddfd0481cc6897"
}
```

| field | meaning |
|---|---|
| `schema` | must be `1`; a client that does not recognise it refuses rather than guesses |
| `version` | compared numerically, not as a string, so `0.1.10` is newer than `0.1.9` |
| `url` | must be `https://`; a plain-HTTP download URL is refused |
| `sha256` | 64 hex characters, checked against the downloaded bytes |

An `expires` field may also be present. It is covered in
[why an old index is refused](updates-rollback.md).

## The detached signature

`latest.json.sig` is a detached GPG signature over the **raw bytes** of
`latest.json` — checked before the bytes are parsed, so a clever index cannot
hide anything by being malformed.

```
latest.json       the index
latest.json.sig   its signature
```

Both are served by every mirror. A mirror that serves one without the other is
broken, and a client that finds only one says so rather than proceeding.

## The pinned key

`keys/camarelol.pub` is embedded into the binary at build time with
`include_str!`, so it travels with every copy of camareLOL. There is no
configuration step and no keyring to tamper with.

The check is that the key that **signed** is **equal to** the key that is
pinned. Not "a key GPG was willing to trust" — a keyring that already contains
a key an attacker added would satisfy any weaker test.

| | |
|---|---|
| Where the key lives | `keys/camarelol.pub`, embedded at build time |
| Keyring used for verification | a temporary directory; your own GPG keyring is never touched |
| `EXPKEYSIG`, `REVKEYSIG`, `BADSIG` | refused |
| Signature over different bytes | refused |

## What happens without a signature

An unsigned index is refused, and the refusal is not a warning:

```console
$ camarelol --update
error: [update_signature_missing] refusing to install an update index that is not signed
  (no public key is configured, so the index cannot be authenticated)
  → publish latest.json.sig and configure a pinned key, or re-run with
    --allow-unsigned-update if you accept the risk
```

`--allow-unsigned-update` exists because a packager behind a corporate proxy
may have no way to serve a signature. It has to be typed. The default is to do
nothing.

For a tool that erases disks, an update that cannot be authenticated is not
worth installing.

## Rotating the key

When a key is compromised, replacing it is not enough on its own — binaries
already in the field carry the old key. The sequence is:

1. Generate the replacement and add it to `keys/camarelol.pub`.
2. Build and sign a release with the new key.
3. Publish. The old key stops signing anything, so the mirror cannot serve
   anything new under it.

This happened once already. The compromised key was deleted, a new one signed
v0.1.10 onward, and the revocation is why an old mirror cannot hand a current
client something signed by the compromised key.

## For maintainers

```console
$ ./scripts/make-signing-key.sh          # once, on your own machine
$ ./scripts/make-release.sh 0.1.10       # builds from the tag, refuses a dirty tree
$ ./scripts/sign-release.sh 0.1.10       # signs, then verifies with the pinned key
```

`sign-release.sh` verifies its own output against `keys/camarelol.pub` before
it reports success, so a signature made with the wrong key cannot be uploaded by
accident.

## Next

- [Why an old index is refused](updates-rollback.md)
- [Building your own package](updates-packaging.md)
- [When an update is refused](updates-troubleshooting.md)