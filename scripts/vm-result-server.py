#!/usr/bin/env python3
"""Thu kết quả từ máy khách VirtualBox về máy chủ.

    python scripts/vm-result-server.py --port 8899 --out /tmp/opencode/vm-results

Bộ test cũ chụp ảnh màn hình rồi coi như xong: không có chỗ nào đọc nội dung
ảnh, nên `verify` không thể thất bại — nó luôn trả 0. Một bài kiểm tra không thể
thất bại không kiểm tra gì.

Cách sửa ở đây không cần Guest Additions. Máy khách có mạng NAT, mà địa chỉ
10.0.2.2 là máy chủ. Khách POST kết quả về đây sau mỗi bước, nên nếu một bước
treo thì các bước trước vẫn còn kết quả — và nếu khách chết giữa chừng thì ta
biết nó chết ở đâu chứ không phải "không có gì".

Máy chủ cũng phục vụ `GET /camarelol` để khách tải đúng binary cần thử, thay vì
tải từ internet. Bài kiểm tra lúc đó không còn phụ thuộc mạng ngoài, và cũng
không phụ thuộc việc release đã được ai đó tải về.
"""

from __future__ import annotations

import argparse
import http.server
import pathlib
import socketserver
import sys
import threading


class Handler(http.server.BaseHTTPRequestHandler):
    out: pathlib.Path = pathlib.Path("/tmp/opencode/vm-results")
    files: pathlib.Path = pathlib.Path("/tmp/opencode/vm-files")

    def _send(self, code: int, body: bytes, ctype: str) -> None:
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802 - tên do BaseHTTPRequestHandler quy định
        # `/camarelol` và `/vm-helper.py`: phục vụ đúng tệp cần thử, không phải
        # bất cứ thứ gì đang nằm trên máy chủ.
        name = self.path.lstrip("/")
        # GET cũng phải thành tiếng: bước tải binary là bước đầu tiên của khách
        # — nếu nó không hiện ở đây thì máy khách chưa từng tới được máy chủ,
        # và bài kiểm tra thất bại ở mạng chứ không phải ở binary.
        print(f"  GET {self.path}", flush=True)
        if "/" in name or not name:
            self._send(404, b"not found", "text/plain")
            return
        path = Handler.files / name
        if path.is_file():
            self._send(200, path.read_bytes(), "application/octet-stream")
        else:
            self._send(404, b"not found", "text/plain")

    def do_POST(self) -> None:  # noqa: N802
        length = int(self.headers.get("Content-Length", "0"))
        data = self.rfile.read(length) if length else b""

        # `/step/<tên>` -> tệp riêng. `/all` -> gộp vào một tệp.
        name = self.path.strip("/").replace("/", "_") or "result"
        if name in ("", "favicon.ico"):
            self._send(400, b"bad path", "text/plain")
            return

        target = self.out / f"{name}.txt"
        self.out.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        print(f"  nhận {name}: {len(data)} byte", flush=True)
        self._send(200, b"ok", "text/plain")

    def log_message(self, *_args) -> None:
        """Im lặng. Mỗi yêu cầu lỗi không liên quan sẽ thành tiếng ồn."""


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=8899)
    ap.add_argument("--out", default="/tmp/opencode/vm-results")
    ap.add_argument("--serve-dir", required=True,
                    help="thư mục phục vụ qua GET, ví dụ /camarelol, /vm-helper.py")
    args = ap.parse_args()

    out = pathlib.Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    for old in out.glob("*.txt"):
        old.unlink()

    Handler.out = out
    Handler.files = pathlib.Path(args.serve_dir).resolve()
    if not Handler.files.is_dir():
        print(f"  không có thư mục phục vụ: {Handler.files}", file=sys.stderr)
        return 1
    served = sorted(p.name for p in Handler.files.iterdir() if p.is_file())
    print(f"  phục vụ {len(served)} tệp từ {Handler.files}: {', '.join(served)}",
          flush=True)
    socketserver.TCPServer.allow_reuse_address = True
    with socketserver.TCPServer(("0.0.0.0", args.port), Handler) as httpd:
        print(f"  lắng nghe 0.0.0.0:{args.port}, kết quả về {out}", flush=True)
        httpd.serve_forever()
    return 0


if __name__ == "__main__":
    sys.exit(main())
