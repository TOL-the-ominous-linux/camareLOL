#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# Tạo GitHub release từ out/release/ và tải mọi asset lên.
#
#   ./scripts/upload-release.sh 1.0.1
#   ./scripts/upload-release.sh 1.0.1 --prerelease
#
# Nội dung release đọc từ docs/releases/<phiên bản>.md. Không hard-code ghi
# chú của một bản cụ thể trong script này nữa: bản 0.1.22 từng nằm trong
# thân script và bản sau chỉ cần quên sửa là release mới mô tả bản cũ.
#
# Nên chạy sau ./scripts/make-release.sh (cùng phiên bản) — script này coi
# out/release/ là nguồn sự thật: binary, tarball mã nguồn, catalog jar,
# PKGBUILD, SHA256SUMS.
#
# Release tạo ở chế độ nháp, mọi asset lên đủ rồi mới publish — upload đứt
# giữa chừng không bao giờ cho người dùng thấy nửa bản phát hành.
#
# Token đọc từ biến CAMARELOL_GH_TOKEN, hoặc tệp CAMARELOL_GH_TOKEN_FILE,
# hoặc một trong các đường dẫn quen thuộc bên dưới (dòng token đầu tiên).
# ---------------------------------------------------------------------------
set -euo pipefail

REPO_DIR="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$REPO_DIR/out/release"

VERSION="${1:-}"
[ -n "$VERSION" ] || { echo "cách dùng: $0 <phiên bản> [--prerelease]" >&2; exit 1; }
VERSION="${VERSION#v}"
TAG="v$VERSION"
PRERELEASE=0
for arg in "${@:2}"; do
    case "$arg" in
        --prerelease|--pre) PRERELEASE=1 ;;
        --release)          PRERELEASE=0 ;;
        *) echo "tham số không hiểu: $arg" >&2; exit 1 ;;
    esac
done

NOTES="$REPO_DIR/docs/releases/$VERSION.md"
[ -f "$NOTES" ] || {
    echo "không có ghi chú phát hành: $NOTES" >&2
    echo "tạo tệp đó trước — upload-release không tự bịa nội dung." >&2
    exit 1
}
[ -s "$NOTES" ] || { echo "ghi chú phát hành rỗng: $NOTES" >&2; exit 1; }

API="https://api.github.com/repos/tamkogay9876-code/camareLOL/releases"
REPO_API="https://api.github.com/repos/tamkogay9876-code/camareLOL"

TOKEN="${CAMARELOL_GH_TOKEN:-}"
if [ -z "$TOKEN" ]; then
    for f in \
        "${CAMARELOL_GH_TOKEN_FILE:-}" \
        "$HOME/Tài liệu/cắt cy/github.txt" \
        "/run/media/xogfredia/Storage/ANY!/SIRO/cắt cy/github.txt"
    do
        [ -n "$f" ] || continue
        [ -f "$f" ] || continue
        TOKEN="$(grep -E '^(gh[pousr]_|github_pat_)' "$f" | head -1 | tr -d '\r\n')"
        [ -n "$TOKEN" ] && break
    done
fi
[ -n "$TOKEN" ] || {
    echo "không có token GitHub: đặt CAMARELOL_GH_TOKEN hoặc trỏ" >&2
    echo "CAMARELOL_GH_TOKEN_FILE vào tệp chứa token." >&2
    exit 1
}

[ -d "$OUT" ] || { echo "không có $OUT — chạy ./scripts/make-release.sh $VERSION trước" >&2; exit 1; }
cd "$OUT"
for f in "camarelol" "camarelol-$VERSION.tar.gz" "camarelol-catalog.jar" "PKGBUILD" "SHA256SUMS"; do
    [ -f "$f" ] || { echo "thiếu asset $f trong out/release/" >&2; exit 1; }
done

# Đừng bao giờ tải lên một bộ tệp mà chính nó không khớp SHA256SUMS. Lần phát
# hành 1.0.0 đầu tiên suýt tải lên bảng băm còn sót từ 0.1.22 vì make-release
# cũ không sinh lại tệp này — kiểm ở đây chặn đúng kiểu lỗi đó.
sha256sum -c SHA256SUMS >/dev/null \
    || { echo "SHA256SUMS không khớp các asset trong out/release/ — dừng." >&2; exit 1; }
grep -q " camarelol-$VERSION.tar.gz$" SHA256SUMS \
    || { echo "SHA256SUMS không có camarelol-$VERSION.tar.gz — dừng." >&2; exit 1; }

# ------------------------------------------------------------------ release cũ?
CODE="$(curl -sS -o /dev/null -w '%{http_code}' \
    -H "Authorization: token $TOKEN" -H "Accept: application/vnd.github+json" \
    "$REPO_API/releases/tags/$TAG")"
case "$CODE" in
    200|201)
        echo "release $TAG đã tồn tại — không ghi đè." >&2
        echo "xoá release cũ trên GitHub nếu thật sự muốn phát hành lại." >&2
        exit 1
        ;;
    404) : ;;
    *) echo "không kiểm tra được release $TAG (HTTP $CODE)" >&2; exit 1 ;;
esac

# ------------------------------------------------------------------ payload
# Ghi chú có thể chứa ", $, backtick, xuống dòng — không nhét vào chuỗi shell.
PAYLOAD="$(tag="$TAG" name="camareLOL $VERSION" notes="$NOTES" \
    pre="$PRERELEASE" python3 -c '
import json, os, sys
body = open(os.environ["notes"], encoding="utf-8").read()
sys.stdout.write(json.dumps({
    "tag_name": os.environ["tag"],
    "name": os.environ["name"],
    "body": body,
    "draft": True,
    "prerelease": os.environ["pre"] == "1",
    "make_latest": "false" if os.environ["pre"] == "1" else "true",
}, ensure_ascii=False))
')"

echo "== tạo nháp release $TAG (prerelease=$PRERELEASE) =="
RELEASE_ID="$(curl -fsS -X POST "$API" \
    -H "Authorization: token $TOKEN" -H "Accept: application/vnd.github+json" \
    --data-binary "$PAYLOAD" \
    | python3 -c 'import json,sys; print(json.load(sys.stdin)["id"])')"
echo "   release id $RELEASE_ID"

# ---------------------------------------------------------------- tải asset
for f in "camarelol" "camarelol-$VERSION.tar.gz" "camarelol-catalog.jar" "PKGBUILD" "SHA256SUMS"; do
    echo "== tải $f =="
    curl -fsS -X POST \
        "https://uploads.github.com/repos/tamkogay9876-code/camareLOL/releases/$RELEASE_ID/assets?name=$f" \
        -H "Authorization: token $TOKEN" -H "Content-Type: application/octet-stream" \
        --data-binary "@$f" \
        | python3 -c 'import json,sys; d=json.load(sys.stdin); print("  →", d["state"], d["browser_download_url"])'
done

# ------------------------------------------------- mọi asset lên đủ mới publish
echo "== publish =="
curl -fsS -X PATCH "$API/$RELEASE_ID" \
    -H "Authorization: token $TOKEN" -H "Accept: application/vnd.github+json" \
    -d '{"draft": false}' \
    | python3 -c 'import json,sys; d=json.load(sys.stdin); print("published:", d["html_url"], "| prerelease:", d["prerelease"])'
