"""Shared by the site tools.

Kept dependency-free and separate from the installer's own `lol/` package: this
directory is downloaded on its own by the mirror's CI, so anything it imports
has to travel with it.
"""

from __future__ import annotations

import json


class BadIndex(Exception):
    """The bytes are not a usable update index."""


def check_shape(index: bytes) -> dict:
    """Reject an index we could not serve or act on.

    Same rules the Rust client applies, so the status page does not report a
    mirror as healthy when the binary would refuse it.
    """
    try:
        doc = json.loads(index)
    except (json.JSONDecodeError, UnicodeDecodeError) as exc:
        raise BadIndex(f"not JSON: {exc}") from exc

    if not isinstance(doc, dict):
        raise BadIndex("the index is not an object")
    for field in ("schema", "version", "url", "sha256"):
        if field not in doc:
            raise BadIndex(f"missing '{field}'")
    if doc["schema"] != 1:
        raise BadIndex(f"unsupported schema {doc['schema']}")
    if not str(doc["url"]).startswith("https://"):
        raise BadIndex("the index points at a non-HTTPS download URL")
    if len(str(doc["sha256"])) != 64:
        raise BadIndex("the index has no usable sha256")
    return doc


def read_mirror_list(path) -> list[str]:
    """Pull the URL strings out of `mirrors.toml`.

    Hand-written rather than a TOML parser so the toolkit stays dependency
    free. It only understands the shape our own file has — a `mirrors = [`
    array of quoted strings — and says so rather than pretending otherwise.
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