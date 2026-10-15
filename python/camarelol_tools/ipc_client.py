"""JSON IPC client for the camareLOL core.

Speaks the newline-delimited JSON protocol from docs/en/ipc-protocol.md:

    {"id":1,"protocol_version":1,"method":"scan_devices","params":{},...}
    {"id":1,"protocol_version":1,"status":"ok","result":{...},"duration_ms":3}

The client is deliberately strict: it refuses a core that speaks a different
protocol version, and it never fabricates a success it did not receive.
"""

from __future__ import annotations

import json
import socket
from dataclasses import dataclass
from typing import Any, Iterator

PROTOCOL_VERSION = 1
DEFAULT_TIMEOUT_MS = 120_000


class ProtocolError(RuntimeError):
    """Raised when the core answers with a protocol violation."""


@dataclass
class Response:
    id: int
    protocol_version: int
    status: str
    result: Any = None
    error: dict | None = None
    duration_ms: int = 0

    @property
    def ok(self) -> bool:
        return self.status == "ok"

    @property
    def error_code(self) -> str | None:
        if self.error:
            return str(self.error.get("code"))
        return None

    def unwrap(self) -> Any:
        if not self.ok:
            raise ProtocolError(f"core returned {self.status}: {self.error}")
        return self.result


class CoreClient:
    """A tiny blocking client. Enough for automation and tests."""

    def __init__(self, host: str = "127.0.0.1", port: int = 47811, timeout_s: float = 30.0):
        self.host = host
        self.port = port
        self.timeout_s = timeout_s
        self._next_id = 0

    # ------------------------------------------------------------------
    def _request(self, method: str, params: dict, timeout_ms: int | None = None) -> Response:
        with socket.create_connection((self.host, self.port), timeout=self.timeout_s) as sock:
            sock.settimeout(self.timeout_s)
            self._next_id += 1
            request = {
                "id": self._next_id,
                "protocol_version": PROTOCOL_VERSION,
                "method": method,
                "params": params,
                "timeout_ms": timeout_ms or DEFAULT_TIMEOUT_MS,
            }
            sock.sendall((json.dumps(request) + "\n").encode())
            data = b""
            while not data.endswith(b"\n"):
                chunk = sock.recv(65536)
                if not chunk:
                    raise ProtocolError("core closed the connection")
                data += chunk

        raw = json.loads(data.decode())
        if raw.get("protocol_version") != PROTOCOL_VERSION:
            raise ProtocolError(
                f"unsupported protocol version {raw.get('protocol_version')!r}"
            )
        if raw.get("id") != request["id"]:
            raise ProtocolError("response id does not match the request id")
        return Response(**{k: v for k, v in raw.items() if k in Response.__annotations__})

    # ------------------------------------------------------------------
    def hello(self) -> Response:
        return self._request("hello", {})

    def scan_devices(self) -> Response:
        return self._request("scan_devices", {})

    def create_plan(self, disk: str) -> Response:
        return self._request("create_plan", {"disk": disk})

    def validate_plan(self, disk: str) -> Response:
        return self._request("validate_plan", {"disk": disk})

    def apply_plan(self, disk: str) -> Response:
        """Destructive. Only works when the core was started armed."""
        return self._request("apply_plan", {"disk": disk})

    def diagnostics(self) -> Response:
        return self._request("diagnostics", {})


def parse_lines(payload: str) -> Iterator[dict]:
    """Parse a stream of NDJSON responses (used by the tests)."""
    for line in payload.splitlines():
        line = line.strip()
        if line:
            yield json.loads(line)


def main(argv: list[str] | None = None) -> int:
    import argparse

    parser = argparse.ArgumentParser(description="query a camareLOL core over JSON IPC")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=47811)
    parser.add_argument("command", choices=["hello", "scan", "plan", "validate", "diagnostics"])
    parser.add_argument("--disk", default="")
    args = parser.parse_args(argv)

    client = CoreClient(args.host, args.port)
    try:
        if args.command == "hello":
            response = client.hello()
        elif args.command == "scan":
            response = client.scan_devices()
        elif args.command == "plan":
            response = client.create_plan(args.disk)
        elif args.command == "validate":
            response = client.validate_plan(args.disk)
        else:
            response = client.diagnostics()
    except (ProtocolError, OSError) as exc:
        print(f"error: {exc}")
        return 2

    print(json.dumps(response.__dict__, indent=2))
    return 0 if response.ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
