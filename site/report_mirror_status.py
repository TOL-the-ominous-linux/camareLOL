#!/usr/bin/env python3
"""Turn mirror health into CI annotations and an exit code.

    python lol/report_mirror_status.py mirror-status.json

Kept out of the workflow file so the YAML holds no embedded Python: a heredoc
inside a block scalar has to be indented to match, and getting that wrong
breaks the file in a way that only shows up when the workflow runs.
"""

from __future__ import annotations

import json
import sys


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2

    with open(argv[1]) as fh:
        status = json.load(fh)

    healthy = status.get("healthy", 0)
    total = status.get("total", 0)
    down = [m["url"] for m in status.get("mirrors", []) if not m.get("ok")]

    for url in down:
        print(f"::warning::mirror is down: {url}")

    if healthy == 0:
        print("::error::every mirror is down; nobody can update")
        return 1

    # One mirror gone is worth a warning, not a failure: the updater falls
    # through to the next one, which is exactly what the list is for.
    print(f"::notice::{healthy}/{total} mirrors healthy")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))