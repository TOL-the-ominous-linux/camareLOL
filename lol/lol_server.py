#!/usr/bin/env python3
"""LOL — the camareLOL update mirror, as a service.

    python3 lol/lol_server.py --host 127.0.0.1 --port 8080

Endpoints:

    GET  /latest.json       the update index
    GET  /latest.json.sig   its detached signature
    GET  /healthz          is the mirror holding anything
    POST /_refresh         re-sync from GitHub (bearer token required)

Only those two files are readable. There is no path handling beyond a lookup in
a two-element tuple, so there is nothing for a traversal attempt to reach.

Why no private key here: the signing key lives in the maintainer's machine and
the verification key is compiled into camareLOL. This process copies two files
that are already signed. Compromising it does not let anyone forge an update —
it can only stop people seeing one, or replay an old one.
"""

from __future__ import annotations

import argparse
import hashlib
import hmac
import json
import os
import secrets
import sys
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from lol_sync import SERVABLE, log, sync  # noqa: E402

MAX_BODY = 1024  # POST /_refresh takes no body; refuse anything large early

_lock = threading.Lock()
_syncing = False


class Handler(BaseHTTPRequestHandler):
    server_version = "lol"
    sys_version = ""

    out_dir: Path
    refresh_token: str
    pubkey: Path
    allow_unsigned: bool

    # ------------------------------------------------------------- helpers
    def _send(self, code: int, body: bytes, ctype: str, extra: dict | None = None) -> None:
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        # The content is public and changes rarely; let intermediaries keep it.
        self.send_header("Cache-Control", "public, max-age=300")
        for k, v in (extra or {}).items():
            self.send_header(k, v)
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(body)

    def _json(self, code: int, doc: dict) -> None:
        self._send(code, json.dumps(doc).encode(), "application/json")

    def _refuse(self, code: int, msg: str) -> None:
        self.log_message("%s", msg)
        self._json(code, {"error": msg})

    # ----------------------------------------------------------------- GET
    def do_GET(self) -> None:  # noqa: N802
        path = self.path.split("?", 1)[0]

        if path == "/healthz":
            have = {name: (self.out_dir / name).exists() for name in SERVABLE}
            code = 200 if all(have.values()) else 503
            self._json(code, {"ok": all(have.values()), "files": have})
            return

        if path not in (f"/{name}" for name in SERVABLE):
            self._refuse(404, "not found")
            return

        name = path.lstrip("/")
        try:
            body = (self.out_dir / name).read_bytes()
        except OSError:
            self._refuse(503, "the mirror has no copy yet")
            return

        etag = '"%s"' % hashlib.sha256(body).hexdigest()[:32]
        if self.headers.get("If-None-Match") == etag:
            self.send_response(304)
            self.send_header("ETag", etag)
            self.end_headers()
            return

        ctype = "application/json" if name.endswith(".json") else "application/pgp-signature"
        self._send(200, body, ctype, {"ETag": etag})

    def do_HEAD(self) -> None:  # noqa: N802
        self.do_GET()

    # ---------------------------------------------------------------- POST
    def do_POST(self) -> None:  # noqa: N802
        if self.path.split("?", 1)[0] != "/_refresh":
            self._refuse(404, "not found")
            return

        # Constant-time compare: a timing side channel here would leak the
        # refresh token byte by byte.
        offered = (self.headers.get("Authorization") or "").removeprefix("Bearer ")
        if not hmac.compare_digest(offered, self.refresh_token):
            self._refuse(401, "bad token")
            return

        length = int(self.headers.get("Content-Length") or 0)
        if length > MAX_BODY:
            self._refuse(413, "no body expected")
            return
        if length:
            self.rfile.read(length)

        global _syncing
        with _lock:
            if _syncing:
                self._refuse(429, "a sync is already running")
                return
            _syncing = True

        started = time.time()
        try:
            doc = sync(self.out_dir, self.pubkey, require_signature=not self.allow_unsigned)
            self._json(200, {"ok": True, "version": doc["version"],
                             "seconds": round(time.time() - started, 2)})
        except Exception as exc:  # noqa: BLE001 - report, never crash the mirror
            log(f"refresh failed: {exc}")
            self._json(502, {"ok": False, "error": str(exc)})
        finally:
            with _lock:
                _syncing = False


def main() -> int:
    repo_root = Path(__file__).resolve().parent.parent
    ap = argparse.ArgumentParser(description="Serve the camareLOL update index")
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=8080)
    ap.add_argument("--out", default=str(repo_root / "lol" / "site"))
    ap.add_argument("--pubkey", default=str(repo_root / "keys" / "camarelol.pub"))
    ap.add_argument("--refresh-token", default=os.environ.get("LOL_REFRESH_TOKEN"))
    ap.add_argument("--allow-unsigned", action="store_true")
    ap.add_argument("--sync-on-start", action="store_true",
                    help="fetch from GitHub before serving, so the mirror is never empty")
    args = ap.parse_args()

    if not args.refresh_token:
        # Generated once and printed once. Restarting therefore invalidates it,
        # which is the right default for a credential nobody chose.
        args.refresh_token = secrets.token_urlsafe(32)
        log(f"no LOL_REFRESH_TOKEN given; generated one for this run: {args.refresh_token}")

    out = Path(args.out)
    Handler.out_dir = out
    Handler.refresh_token = args.refresh_token
    Handler.pubkey = Path(args.pubkey)
    Handler.allow_unsigned = args.allow_unsigned

    if args.sync_on_start:
        try:
            sync(out, Path(args.pubkey), require_signature=not args.allow_unsigned)
        except Exception as exc:  # noqa: BLE001
            log(f"start-up sync failed ({exc}); serving whatever is on disk")

    httpd = ThreadingHTTPServer((args.host, args.port), Handler)
    httpd.daemon_threads = True
    log(f"listening on http://{args.host}:{args.port} serving {out}")
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        log("stopping")
    return 0


if __name__ == "__main__":
    sys.exit(main())