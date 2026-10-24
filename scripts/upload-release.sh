#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# Tạo GitHub release từ out/release/ và tải mọi asset lên.
#
#   ./scripts/upload-release.sh 0.1.19
#
# Nên chạy sau ./scripts/make-release.sh (cùng phiên bản) — script này coi
# out/release/ là nguồn sự thật: binary, tarball mã nguồn, catalog jar,
# PKGBUILD, SHA256SUMS.
#
# Release tạo ở chế độ nháp, mọi asset lên đủ rồi mới publish — upload đứt
# giữa chừng không bao giờ cho người dùng thấy nửa bản phát hành.
#
# Token đọc từ biến CAMARELOL_GH_TOKEN, hoặc tệp
# "$HOME/Tài liệu/cắt cy/github.txt" (dòng token đầu tiên).
# ---------------------------------------------------------------------------
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$REPO_DIR/out/release"

VERSION="${1:-}"
[ -n "$VERSION" ] || { echo "cách dùng: $0 <phiên bản>" >&2; exit 1; }
VERSION="${VERSION#v}"
TAG="v$VERSION"
REPO_HOST="github.com/tamkogay9876-code/camareLOL"
API="https://api.github.com/repos/tamkogay9876-code/camareLOL/releases"

TOKEN="${CAMARELOL_GH_TOKEN:-}"
if [ -z "$TOKEN" ]; then
    TOKEN_FILE="$HOME/Tài liệu/cắt cy/github.txt"
    [ -f "$TOKEN_FILE" ] || { echo "không có token: đặt CAMARELOL_GH_TOKEN hoặc tạo $TOKEN_FILE" >&2; exit 1; }
    TOKEN="$(grep -E '^(gh[pousr]_|github_pat_)' "$TOKEN_FILE" | head -1 | tr -d '\r\n')"
    [ -n "$TOKEN" ] || { echo "không thấy token gh…/github_pat_… trong $TOKEN_FILE" >&2; exit 1; }
fi

[ -d "$OUT" ] || { echo "không có $OUT — chạy ./scripts/make-release.sh $VERSION trước" >&2; exit 1; }
cd "$OUT"
for f in "camarelol" "camarelol-$VERSION.tar.gz" "camarelol-catalog.jar" "PKGBUILD" "SHA256SUMS"; do
    [ -f "$f" ] || { echo "thiếu asset $f trong out/release/" >&2; exit 1; }
done

BODY="camareLOL $VERSION.

Highlights:
- Real-machine dual-boot feedback, 23-24 Oct 2026:

  - the Storage devices screen now shows the UNPARTITIONED GAP next to the
    partition list — the gap-finding logic inside the dual-boot planner is
    now a shared API (largest_aligned_gap / GapScan), so the planner and
    the screen read the disk with the same eyes. A deleted first partition
    leaves a gap at the very START of the disk and the scan finds it
    exactly there (test reproduces that shape).
  - selecting the Dual-boot disk mode now switches the bootloader to GRUB
    automatically, with a message explaining why — the planner's refusal
    (\"systemd-boot cannot share a Windows ESP\") arrives as prevention
    instead of an error three screens later. The DiskMode screen also
    labels what mode 3 keeps.
  - scripts/dual-boot-auto.sh — one-command real-machine install from a
    live ISO: layout gates (GPT + small ESP + big ext4 root, nothing
    mounted), typed ERASE-/EXECUTE confirmations, nothing ever touches
    /dev/nvme*, the ESP, or sgdisk --zap-all.
- Everything 0.1.21 delivered (expanded partition list on the Storage
  screen) and 0.1.20 (keep-everything dual-boot planner, stale EFI entry
  cleanup, NetworkManager dhcp=internal) carries over.

Assets are checksummed in SHA256SUMS."

# ---- tạo nháp release ------------------------------------------------------
PAYLOAD="$(tag="$TAG" name="camareLOL $VERSION" CAMARELOL_BODY="$BODY" python3 -c '
import json, os, sys
sys.stdout.write(json.dumps({
    "tag_name": os.environ["tag"],
    "name": os.environ["name"],
    "body": os.environ["CAMARELOL_BODY"],
    "draft": True, "prerelease": False,
}))
')"
echo "== tạo nháp release $TAG =="
RELEASE_ID="$(curl -fsS -X POST "$API" \
    -H "Authorization: token $TOKEN" -H "Accept: application/vnd.github+json" \
    --data-binary "$PAYLOAD" \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')"
echo "   release id $RELEASE_ID"

# ---- tải từng asset --------------------------------------------------------
for f in "camarelol" "camarelol-$VERSION.tar.gz" "camarelol-catalog.jar" "PKGBUILD" "SHA256SUMS"; do
    echo "== tải $f =="
    curl -fsS -X POST \
        "https://uploads.github.com/repos/tamkogay9876-code/camareLOL/releases/$RELEASE_ID/assets?name=$f" \
        -H "Authorization: token $TOKEN" -H "Content-Type: application/octet-stream" \
        --data-binary "@$f" \
        | python3 -c 'import json,sys; d=json.load(sys.stdin); print("  →", d["state"], d["browser_download_url"])'
done

# ---- mọi asset lên đủ mới công bố -------------------------------------------
echo "== publish =="
curl -fsS -X PATCH "$API/$RELEASE_ID" \
    -H "Authorization: token $TOKEN" -H "Accept: application/vnd.github+json" \
    -d '{"draft": false}' \
    | python3 -c 'import json,sys; print("published:", json.load(sys.stdin)["html_url"])'
