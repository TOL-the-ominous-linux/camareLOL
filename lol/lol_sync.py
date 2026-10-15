#!/usr/bin/env python3
"""LOL — the camareLOL update mirror.

Keeps the update index reachable at one short URL, so a user (or a curl in a
script) does not have to know where in GitHub the newest release lives, and so
the tool keeps working if github.com is slow or blocked.

    https://camarelol.github.io/latest.json
    https://camarelol.github.io/latest.json.sig

## What a mirror can and cannot break

The trust anchor is the signing key, and that key is compiled into the
camareLOL binary. It is not fetched from here, and it is not served from here.
So:

  - A tampered index is rejected. Editing `sha256` or `url` invalidates the
    signature, and the binary refuses before downloading anything.
  - A mirror serving a *different, older* but correctly signed index is not a
    tampering attack, but it does hide security fixes. That is a rollback, and
    it is the one real weakness of a mirror. See "Rollback" in the README.

Because the signature is checked by the client, this service never needs
write access to anything and never holds a private key. It copies two files.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import ssl
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

OWNER = "tamkogay9876-code"
REPO = "camareLOL"
API = f"https://api.github.com/repos/{OWNER}/{REPO}/releases"

# Only these two filenames are ever served. Anything else is a 404, so a
# traversal attempt has nothing to traverse into.
SERVABLE = ("latest.json", "latest.json.sig")

TIMEOUT = 30


def log(msg: str) -> None:
    print(f"[lol] {msg}", flush=True)


def fetch(url: str, accept: str = "application/vnd.github+json") -> bytes:
    req = urllib.request.Request(
        url,
        headers={"Accept": accept, "User-Agent": "camareLOL-lol/1.0"},
    )
    token = os.environ.get("GITHUB_TOKEN")
    if token:
        req.add_header("Authorization", f"Bearer {token}")
    # Verify TLS by default. Silently trusting an unknown CA here would let
    # anyone on the path rewrite the index, which is the entire threat this
    # mirror exists to be careful about.
    ctx = ssl.create_default_context()
    with urllib.request.urlopen(req, timeout=TIMEOUT, context=ctx) as resp:
        return resp.read()


def parse_version(tag: str) -> tuple[int, int, int]:
    """`v0.1.9` -> (0, 1, 9). Unparseable tags sort lowest."""
    m = re.fullmatch(r"v?(\d+)\.(\d+)\.(\d+)", tag.strip())
    if not m:
        return (0, 0, 0)
    return tuple(int(g) for g in m.groups())  # type: ignore[return-value]


def newest_release() -> dict:
    """The highest published release, ignoring drafts and pre-releases."""
    releases = json.loads(fetch(API))
    if not isinstance(releases, list) or not releases:
        raise RuntimeError("GitHub returned no releases")
    candidates = [
        r for r in releases if not r.get("draft") and not r.get("prerelease")
    ]
    if not candidates:
        raise RuntimeError("no published, non-prerelease releases")
    best = max(candidates, key=lambda r: parse_version(r["tag_name"]))
    log(f"newest published release: {best['tag_name']}")
    return best


def index_urls(release: dict) -> tuple[str, str]:
    base = f"https://github.com/{OWNER}/{REPO}/releases/download/{release['tag_name']}"
    return f"{base}/latest.json", f"{base}/latest.json.sig"


def verify_signature(index: bytes, sig: bytes, pubkey_path: Path) -> None:
    """Check the index against the key the repository pins.

    The mirror is not the trust anchor, so this is a sanity check that the files
    it is about to publish are the ones the project actually signed — it stops
    a bad release from being mirrored, rather than stopping an attacker.
    """
    if not pubkey_path.exists():
        raise RuntimeError(f"missing public key: {pubkey_path}")
    if b"PRIVATE KEY BLOCK" in pubkey_path.read_bytes():
        raise RuntimeError("the file that should hold a public key contains a private key")

    with tempfile.TemporaryDirectory() as tmp:
        home = Path(tmp) / "gnupg"
        home.mkdir(mode=0o700)
        (home / "key.pub").write_bytes(pubkey_path.read_bytes())
        (home / "index.json").write_bytes(index)
        (home / "index.sig").write_bytes(sig)

        subprocess.run(
            ["gpg", "--homedir", str(home), "--batch", "--quiet", "--import",
             str(home / "key.pub")],
            check=True, capture_output=True,
        )
        listing = subprocess.run(
            ["gpg", "--homedir", str(home), "--batch", "--with-colons", "--list-keys"],
            check=True, capture_output=True, text=True,
        ).stdout
        fingerprint = next(
            (line.split(":")[9] for line in listing.splitlines() if line.startswith("fpr")),
            None,
        )
        if not fingerprint:
            raise RuntimeError("the pinned public key has no fingerprint")

        out = subprocess.run(
            ["gpg", "--homedir", str(home), "--batch", "--quiet",
             "--status-fd", "1", "--verify", str(home / "index.sig"), str(home / "index.json")],
            capture_output=True, text=True,
        )
        status = out.stdout
        if "VALIDSIG" not in status or fingerprint.upper() not in status.upper():
            raise RuntimeError(
                "the index is not signed by the key this repository pins — "
                "refusing to mirror it"
            )
        log(f"signature verified against pinned key {fingerprint}")


def default_expiry(valid_days: int = 7) -> int:
    """When a mirrored index should stop being accepted.

    Seven days is longer than the mirror refreshes, so a healthy mirror never
    serves anything near expiry. It exists to stop a taken-over mirror from
    replaying an old but validly signed index and hiding security fixes — the
    one thing a mirror can do that it is not allowed to.
    """
    return int(time.time()) + valid_days * 86_400


def check_shape(index: bytes) -> dict:
    """Reject an index we could not serve, before publishing it."""
    doc = json.loads(index)
    for field in ("schema", "version", "url", "sha256"):
        if field not in doc:
            raise RuntimeError(f"the index is missing '{field}'")
    if doc["schema"] != 1:
        raise RuntimeError(f"unsupported index schema {doc['schema']}")
    if not str(doc["url"]).startswith("https://"):
        raise RuntimeError("the index points at a non-HTTPS download URL")
    if len(str(doc["sha256"])) != 64:
        raise RuntimeError("the index has no usable sha256")
    return doc


def publish(out_dir: Path, index: bytes, sig: bytes) -> None:
    """Write both files, replacing them only once both are in place.

    Half a publish — index without signature, or the reverse — makes every
    client refuse with a confusing error, so both land together.
    """
    out_dir.mkdir(parents=True, exist_ok=True)
    staging = Path(tempfile.mkdtemp(dir=out_dir, prefix=".staging-"))
    try:
        (staging / "latest.json").write_bytes(index)
        (staging / "latest.json.sig").write_bytes(sig)
        for name in SERVABLE:
            os.replace(staging / name, out_dir / name)
    finally:
        shutil.rmtree(staging, ignore_errors=True)


def fetch_signature(sig_url: str, attempts: int = 6, wait: float = 20.0) -> bytes:
    """Wait for the signature, which is uploaded after the release appears.

    A release is published first and signed afterwards — that is the whole
    order, because the index cannot be signed until the release exists to
    point at. Anything triggered by `release: published` therefore runs in the
    window where the signature is not there yet, and fails with a 404 that
    looks like a broken mirror rather than a release still being signed.

    Retrying is the difference between a mirror that catches up on its own and
    one that waits for tomorrow's scheduled run.
    """
    last = ""
    for attempt in range(1, attempts + 1):
        try:
            return fetch(sig_url, accept="application/octet-stream")
        except (urllib.error.URLError, OSError) as exc:
            last = str(exc)
            log(f"signature not there yet ({attempt}/{attempts}): {exc}")
            if attempt < attempts:
                time.sleep(wait)
    raise RuntimeError(
        f"no signature at {sig_url} after {attempts} attempts ({wait:g}s apart).\n"
        f"This release is still being signed. If it is not, the signature upload failed\n"
        f"and nothing should be mirrored from it."
    )


def sync(out_dir: Path, pubkey_path: Path, require_signature: bool = True) -> dict:
    release = newest_release()
    index_url, sig_url = index_urls(release)
    index = fetch(index_url, accept="application/octet-stream")
    log(f"fetched index ({len(index)} bytes) from {index_url}")

    doc = check_shape(index)

    if require_signature:
        sig = fetch_signature(sig_url)
        verify_signature(index, sig, pubkey_path)
    else:
        sig = b""
        log("WARNING: signature not checked; this release will be refused by every client")

    publish(out_dir, index, sig)
    log(f"published v{doc['version']} to {out_dir}")
    return doc


def main() -> int:
    repo_root = Path(__file__).resolve().parent.parent
    ap = argparse.ArgumentParser(description="Sync the camareLOL update index")
    ap.add_argument("--out", default=os.environ.get("LOL_OUT", str(repo_root / "lol" / "site")),
                    help="directory the two files are written to")
    ap.add_argument("--pubkey", default=os.environ.get(
        "LOL_PUBKEY", str(repo_root / "keys" / "camarelol.pub")))
    ap.add_argument("--allow-unsigned", action="store_true",
                    help="mirror even without a valid signature (clients will refuse it)")
    args = ap.parse_args()

    try:
        sync(Path(args.out), Path(args.pubkey), require_signature=not args.allow_unsigned)
    except (RuntimeError, urllib.error.URLError, json.JSONDecodeError,
            subprocess.CalledProcessError, OSError) as exc:
        log(f"sync failed: {exc}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())