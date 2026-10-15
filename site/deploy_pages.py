#!/usr/bin/env python3
"""Deploy bản site + mirror lên Cloudflare Pages.

    python site/deploy_pages.py --manifest /tmp/manifest.json

Đọc token từ biến môi trường `CLOUDFLARE_API_TOKEN`. Trong CI nó là secret của
kho; trên máy này nó nằm trong thư mục cách ly và được nạp vào môi trường.

Không dùng thư viện: một lệnh curl, và không có thêm thứ gì phải cài đặt hay
phải giữ cập nhật. Người đọc có thể thấy chính xác điều gì được gửi đi.

Vì sao deploy qua API thay vì nối kho với Cloudflare: nối kho cần bước ủy quyền
trên trình duyệt mà tôi không thao tác được. Deploy bằng API thì bất kỳ lần nào
workflow chạy, nó cũng đăng — và nếu Cloudflare không nhận, bước này hỏng và
công việc bị báo hỏng chứ không phải im lặng.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.request
from pathlib import Path

API = "https://api.cloudflare.com/client/v4"


def call(method: str, url: str, token: str, data: dict | None = None,
         files: dict | None = None, headers: dict | None = None) -> dict:
    """Gọi JSON. `files` dùng multipart — Cloudflare nhận manifest như tệp."""
    boundary = "----camarelolmanifest"
    if files:
        parts = []
        for key, value in (data or {}).items():
            parts.append(f"--{boundary}\r\nContent-Disposition: form-data; name=\"{key}\"\r\n\r\n{value}\r\n")
        for key, (name, content, ctype) in files.items():
            parts.append(
                f"--{boundary}\r\nContent-Disposition: form-data; name=\"{key}\"; "
                f"filename=\"{name}\"\r\nContent-Type: {ctype}\r\n\r\n{content}\r\n")
        parts.append(f"--{boundary}--\r\n")
        body = "".join(parts).encode("utf-8")
        head = {"Content-Type": f"multipart/form-data; boundary={boundary}"}
    else:
        body = json.dumps(data or {}).encode("utf-8")
        head = {"Content-Type": "application/json"}

    head["Authorization"] = f"Bearer {token}"
    head.update(headers or {})

    req = urllib.request.Request(url, data=body, headers=head, method=method)
    try:
        with urllib.request.urlopen(req, timeout=180) as resp:
            return json.loads(resp.read())
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", "replace")
        try:
            return json.loads(detail)
        except json.JSONDecodeError:
            return {"success": False, "errors": [{"message": detail[:200]}]}


def main() -> int:
    ap = argparse.ArgumentParser(description="Deploy camareLOL to Cloudflare Pages")
    ap.add_argument("--manifest", required=True)
    ap.add_argument("--account", default=os.environ.get("CLOUDFLARE_ACCOUNT_ID", ""))
    ap.add_argument("--project", default="camarelol")
    ap.add_argument("--branch", default="main")
    args = ap.parse_args()

    token = os.environ.get("CLOUDFLARE_API_TOKEN", "").strip()
    if not token:
        print("  CLOUDFLARE_API_TOKEN chưa được đặt", file=sys.stderr)
        return 1
    if not args.account:
        print("  CLOUDFLARE_ACCOUNT_ID chưa được đặt", file=sys.stderr)
        return 1

    manifest = Path(args.manifest).read_text(encoding="utf-8")
    files = json.loads(manifest).get("files", [])
    log = print
    log(f"  manifest: {len(files)} tệp, {len(manifest)} byte")

    url = f"{API}/accounts/{args.account}/pages/projects/{args.project}/deployments"
    result = call("POST", url, token,
                  data={"branch": args.branch,
                        "commit_message": os.environ.get(
                            "COMMIT_MESSAGE", "camareLOL mirror and site")},
                  files={"manifest": ("manifest.json", manifest, "application/json")})

    if not result.get("success"):
        for err in result.get("errors", []):
            log(f"  lỗi: {err.get('message')}")
        return 1

    deployment = result["result"]
    log(f"  deploy id: {deployment.get('id')}")
    log(f"  url: {deployment.get('url')}")
    return 0


if __name__ == "__main__":
    sys.exit(main())