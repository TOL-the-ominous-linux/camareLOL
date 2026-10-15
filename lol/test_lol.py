#!/usr/bin/env python3
"""Tests for the LOL mirror.

The signature check is the one thing here that must not be wrong in the
forgiving direction, so it is tested against a real signature made with a
throwaway key rather than a fixture that only looks right.

    python3 -m pytest lol/test_lol.py      (if pytest is installed)
    python3 lol/test_lol.py                (no dependencies needed)
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
import unittest
import urllib.error
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import lol_sync
from lol_sync import check_shape, parse_version, verify_signature, publish, SERVABLE

HAVE_GPG = subprocess.run(["which", "gpg"], capture_output=True).returncode == 0


def _keypair(home: Path):
    """Create a throwaway signing key. Returns (fingerprint, public_key_path)."""
    subprocess.run(
        ["gpg", "--homedir", str(home), "--batch", "--quiet", "--passphrase", "",
         "--quick-generate-key", "LOL Test <test@example.invalid>", "ed25519", "sign", "never"],
        check=True, capture_output=True,
    )
    fpr = subprocess.run(
        ["gpg", "--homedir", str(home), "--batch", "--with-colons", "--list-keys"],
        check=True, capture_output=True, text=True,
    ).stdout
    fpr = next(l.split(":")[9] for l in fpr.splitlines() if l.startswith("fpr"))
    pub = home / "pub.asc"
    pub.write_bytes(subprocess.run(
        ["gpg", "--homedir", str(home), "--batch", "--armor", "--export", fpr],
        check=True, capture_output=True,
    ).stdout)
    return fpr, pub


def _sign(home: Path, index: bytes, fpr: str) -> bytes:
    idx = home / "index.json"
    sig = home / "index.sig"
    idx.write_bytes(index)
    subprocess.run(
        ["gpg", "--homedir", str(home), "--batch", "--quiet", "--pinentry-mode",
         "loopback", "--passphrase", "", "--local-user", fpr,
         "--detach-sign", "--armor", "-o", str(sig), str(idx)],
        check=True, capture_output=True,
    )
    return sig.read_bytes()


class TestVersionParsing(unittest.TestCase):
    def test_numeric_order_not_alphabetical(self):
        self.assertGreater(parse_version("v0.1.10"), parse_version("v0.1.9"))
        self.assertGreater(parse_version("v0.2.0"), parse_version("v0.1.99"))

    def test_unparseable_tags_sort_lowest(self):
        # A tag we do not understand must never be treated as newest.
        self.assertEqual(parse_version("nightly"), (0, 0, 0))
        self.assertEqual(parse_version("v1.2.3"), (1, 2, 3))

    def test_a_v_prefix_is_optional(self):
        self.assertEqual(parse_version("1.2.3"), parse_version("v1.2.3"))


class TestIndexShape(unittest.TestCase):
    def good(self) -> bytes:
        return json.dumps({
            "schema": 1, "version": "1.0.0",
            "url": "https://example.invalid/camarelol",
            "sha256": "a" * 64,
        }).encode()

    def test_a_well_formed_index_passes(self):
        self.assertEqual(check_shape(self.good())["version"], "1.0.0")

    def test_a_missing_field_is_rejected(self):
        for field in ("schema", "version", "url", "sha256"):
            doc = json.loads(self.good())
            del doc[field]
            with self.assertRaises(RuntimeError, msg=field):
                check_shape(json.dumps(doc).encode())

    def test_a_non_https_download_url_is_rejected(self):
        # The mirror would republish a plain-HTTP download URL to everyone.
        doc = json.loads(self.good())
        doc["url"] = "http://example.invalid/camarelol"
        with self.assertRaises(RuntimeError):
            check_shape(json.dumps(doc).encode())

    def test_a_short_checksum_is_rejected(self):
        doc = json.loads(self.good())
        doc["sha256"] = "abc"
        with self.assertRaises(RuntimeError):
            check_shape(json.dumps(doc).encode())

    def test_a_future_schema_is_rejected_rather_than_guessed(self):
        doc = json.loads(self.good())
        doc["schema"] = 99
        with self.assertRaises(RuntimeError):
            check_shape(json.dumps(doc).encode())


class TestPublish(unittest.TestCase):
    def test_both_files_appear_together(self):
        # Half a publish makes every client refuse with a confusing error.
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            publish(out, b"{}", b"sig")
            self.assertEqual(sorted(p.name for p in out.iterdir()), sorted(SERVABLE))
            self.assertEqual((out / "latest.json").read_bytes(), b"{}")

    def test_publishing_leaves_no_staging_directory(self):
        with tempfile.TemporaryDirectory() as tmp:
            publish(Path(tmp), b"{}", b"sig")
            # iterdir() gives no ordering guarantee; only the set matters here.
            self.assertEqual(sorted(p.name for p in Path(tmp).iterdir()), sorted(SERVABLE))

    def test_an_earlier_pair_is_replaced_not_appended(self):
        with tempfile.TemporaryDirectory() as tmp:
            out = Path(tmp)
            publish(out, b"old", b"oldsig")
            publish(out, b"new", b"newsig")
            self.assertEqual((out / "latest.json").read_bytes(), b"new")
            self.assertEqual((out / "latest.json.sig").read_bytes(), b"newsig")


@unittest.skipUnless(HAVE_GPG, "gpg is not installed")
class TestSignatureVerification(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.home = Path(self.tmp.name) / "gnupg"
        self.home.mkdir(mode=0o700)
        self.fpr, self.pub = _keypair(self.home)
        self.index = json.dumps({
            "schema": 1, "version": "1.0.0",
            "url": "https://example.invalid/camarelol",
            "sha256": "b" * 64,
        }).encode()

    def tearDown(self):
        self.tmp.cleanup()

    def test_a_genuine_signature_is_accepted(self):
        sig = _sign(self.home, self.index, self.fpr)
        verify_signature(self.index, sig, self.pub)  # must not raise

    def test_an_edited_index_is_rejected(self):
        # The whole point: a mirror that rewrites the index must not be able to
        # get its version published.
        sig = _sign(self.home, self.index, self.fpr)
        edited = self.index.replace(b"1.0.0", b"6.6.6")
        self.assertNotEqual(edited, self.index)
        with self.assertRaises(RuntimeError):
            verify_signature(edited, sig, self.pub)

    def test_a_signature_from_another_key_is_rejected(self):
        other_home = Path(self.tmp.name) / "other"
        other_home.mkdir(mode=0o700)
        _, other_pub = _keypair(other_home)
        sig = _sign(self.home, self.index, self.fpr)
        with self.assertRaises(RuntimeError):
            verify_signature(self.index, sig, other_pub)

    def test_a_private_key_where_a_public_one_belongs_is_refused(self):
        # Guards against a maintainer pointing LOL at the wrong file and
        # quietly publishing their private key.
        secret = Path(self.tmp.name) / "secret.asc"
        secret.write_bytes(subprocess.run(
            ["gpg", "--homedir", str(self.home), "--batch", "--armor",
             "--export-secret-keys", self.fpr],
            check=True, capture_output=True,
        ).stdout)
        with self.assertRaises(RuntimeError):
            verify_signature(self.index, b"", secret)

    def test_a_missing_key_file_is_refused(self):
        with self.assertRaises(RuntimeError):
            verify_signature(self.index, b"", Path(self.tmp.name) / "nope.pub")


class TestWaitingForTheSignature(unittest.TestCase):
    """A release is published before it is signed.

    Anything triggered by `release: published` runs in the window where the
    signature does not exist yet. It has to wait rather than report a broken
    mirror.
    """

    def test_it_waits_for_a_signature_that_shows_up_late(self):
        calls = []

        def fake_fetch(url, accept=None):
            calls.append(url)
            if len(calls) < 3:
                raise urllib.error.URLError("404")
            return b"SIGNATURE"

        saved_fetch, saved_sleep = lol_sync.fetch, lol_sync.time.sleep
        lol_sync.fetch, lol_sync.time.sleep = fake_fetch, lambda _s: None
        try:
            self.assertEqual(lol_sync.fetch_signature("u", wait=0), b"SIGNATURE")
        finally:
            lol_sync.fetch, lol_sync.time.sleep = saved_fetch, saved_sleep
        self.assertEqual(len(calls), 3)

    def test_it_gives_up_with_a_message_that_says_why(self):
        def always_missing(url, accept=None):
            raise urllib.error.URLError("404")

        saved_fetch, saved_sleep = lol_sync.fetch, lol_sync.time.sleep
        lol_sync.fetch, lol_sync.time.sleep = always_missing, lambda _s: None
        try:
            with self.assertRaises(RuntimeError) as caught:
                lol_sync.fetch_signature("https://e.invalid/sig", attempts=3, wait=0)
        finally:
            lol_sync.fetch, lol_sync.time.sleep = saved_fetch, saved_sleep
        # A message that just says "404" sends the reader looking for a broken
        # mirror rather than an unfinished release.
        self.assertIn("being signed", str(caught.exception))


if __name__ == "__main__":
    unittest.main(verbosity=2)