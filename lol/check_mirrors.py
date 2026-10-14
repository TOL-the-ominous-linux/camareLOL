#!/usr/bin/env python3
"""Check every mirror and report which ones are alive.

    python3 lol/check_mirrors.py            # human readable
    python3 lol/check_mirrors.py --json     # machine readable

Run from CI on a schedule and publish the result, so a dead mirror is noticed
by the maintainer rather than by a user whose update failed.

A mirror counts as healthy only if it serves an index that parses. Serving a
200 with an HTML error page — which is what a misconfigured SPA host does —
is not healthy, and a status page that only checks HTTP codes would call it
green.
"""

from __future__ import annotations

import argparse
import json
import sys
import urllib.error
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from lol_sync import check_shape  # noqa: E402

REPO = Path(__file__).resolve().parent.parent
TIMEOUT = 25


def read_mirror_list(path: Path) -> list[str]:
    """Pull the URL strings out of `mirrors.toml`.

    Written by hand rather than pulled from the `toml` crate so this runs with
    nothing but the standard library. It only understands the shape our own
    file has — a `mirrors = [` array of quoted strings — and says so rather than
    pretending to parse TOML.
    """
    urls: list[str] = []
    in_list = False
    for line in path.read_text().splitlines():
        stripped = line.strip()
        if not in_list:
            if stripped.startswith("mirrors") and "[" in stripped:
                in_list = True
            continue
        if stripped.startswith("]"):
            break
        if stripped.startswith("#") or not stripped.startswith('"'):
            continue
        parts = stripped.split('"')
        if len(parts) >= 2 and parts[1].strip():
            urls.append(parts[1].strip())
    return urls


def probe(url: str) -> dict:
    """One mirror, one honest verdict."""
    result = {"url": url, "ok": False, "version": None, "detail": ""}
    try:
        req = urllib.request.Request(url, headers={"User-Agent": "camareLOL-mirror-check/1"})
        with urllib.request.urlopen(req, timeout=TIMEOUT) as resp:
            ctype = resp.headers.get("Content-Type", "")
            body = resp.read()
    except (urllib.error.URLError, OSError) as exc:
        result["detail"] = f"unreachable: {exc}"
        return result

    # Judge the content, not the label. GitHub forces application/octet-stream
    # on release assets whatever the filename is, and the client does not care
    # either: a checker that reads Content-Type calls a working mirror dead.
    # What does matter is that the body parses, because a host answering 200
    # with an error page is exactly the failure worth catching.
    try:
        doc = check_shape(body)
    except (RuntimeError, json.JSONDecodeError, UnicodeDecodeError) as exc:
        hint = ""
        if "html" in ctype:
            hint = " (looks like an error page served with 200)"
        result["detail"] = f"not a usable index: {exc}{hint}"
        return result

    result["ok"] = True
    result["version"] = doc["version"]
    return result


def main() -> int:
    ap = argparse.ArgumentParser(description="Check every configured mirror")
    ap.add_argument("--mirrors", default=str(REPO / "mirrors.toml"))
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    urls = read_mirror_list(Path(args.mirrors))
    if not urls:
        print("no mirrors configured", file=sys.stderr)
        return 2

    results = [probe(u) for u in urls]
    healthy = [r for r in results if r["ok"]]

    if args.json:
        print(json.dumps({"healthy": len(healthy), "total": len(results),
                          "mirrors": results}, indent=2))
    else:
        for r in results:
            mark = "OK " if r["ok"] else "DOWN"
            extra = f"v{r['version']}" if r["ok"] else r["detail"]
            print(f"  [{mark}] {r['url']}\n         {extra}")
        print(f"\n{len(healthy)}/{len(results)} mirrors healthy")

    if not healthy:
        print("every mirror is down", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())