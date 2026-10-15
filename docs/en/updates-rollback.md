# Updates: why an old index is refused

[← Documentation index](index.md)

A signature proves **who** signed. It says nothing about **how long ago**.

That gap matters more than it first appears. A compromised mirror can replay a
perfectly valid, correctly signed index from three months ago. Everything about
it verifies — the signature is genuine, the checksum matches, the schema is
current. And a client that accepts it silently stops receiving security fixes.

Signatures do not prevent this on their own. Expiry does.

## The `expires` field

```json
{
  "schema": 1,
  "version": "0.1.10",
  "url": "https://…/camarelol",
  "sha256": "…",
  "expires": 1793299200
}
```

`expires` is a Unix timestamp. The CI sets it to seven days ahead and regenerates
the index on a schedule, so a mirror is never near the limit.

A client refuses an index past its expiry:

```console
$ camarelol --update
error: [update_rollback_blocked] the index expired
```

Seven days is chosen so that a mirror going quiet for a few hours is invisible,
while a mirror frozen for a week is not. It is not a tight budget — it is a
budget.

## Turning the check off

```console
$ camarelol --update --allow-expired-index
```

The flag exists for the case where you are running an air-gapped or
occasionally-connected machine and would rather update on your own schedule.
Like `--allow-unsigned-update`, it has to be typed.

## Older indexes

An index with no `expires` field is still accepted. Refusing it would break
every client the moment the field was introduced, and the clients that matter
most are the ones already installed.

That is a deliberate trade: a missing field means the index predates the
protection, not that it is malicious. Once every mirror emits the field, the
exception could be removed — it is kept for now so an old mirror does not
strand anyone.

## Mirrors do not weaken trust

It is reasonable to ask whether adding mirrors opens a hole. It does not.

The signing key is inside the binary, not in the mirror and not in any
configuration file. A mirror that is edited — to change the URL, to point
somewhere else, to serve an old index — produces a payload the client refuses.
There is no mirror that can authorise anything.

What a mirror can do is delay you: serve stale content, or refuse to answer.
That is why:

- every index is signed, so content is checked regardless of who served it;
- `expires` bounds how stale "stale" may be, so a mirror cannot pin you to an
  old version indefinitely;
- mirrors are tried **in order**, and the list is embedded at build time, so a
  tampered copy on disk cannot add or reorder them.

A mirror that dies is skipped and the next one is tried. Losing all mirrors
means not updating — which is an outage, not a compromise.

## Next

- [Updates: signing](updates-signing.md)
- [Building your own package](updates-packaging.md)
- [When an update is refused](updates-troubleshooting.md)