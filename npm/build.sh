#!/usr/bin/env bash
# Đóng gói @wenker-ai/camarelo cho npm.
#
#   ./npm/build.sh                                   # pack vào npm/
#   ./npm/build.sh publish [tệp-npmrc]               # pack rồi publish
#     ví dụ: ./npm/build.sh publish "$HOME/Tài liệu/cắt cy/npmrc.txt"
#
# Việc đầu tiên là xác định phiên bản: package.json là nguồn sự thật. Binary
# được tải từ GitHub Releases **đúng bản đó** và kiểm SHA-256 so với tệp
# SHA256SUMS phát hành kèm. Nếu package.json lạc sau phát hành, hai lỗi này
# phải hiện rõ instead of lặng lẽ pack một binary sai bản.
#
# Thoát số khác 0 ở bất kỳ bước nào hỏng — một tarball sai binary tuyệt đối
# không được xem là thành công.

set -euo pipefail
cd "$(dirname "$0")"

fail() { printf '  lỗi: %s\n' "$1" >&2; exit 1; }

command -v npm >/dev/null 2>&1 || fail "cần npm trên PATH"
command -v curl >/dev/null 2>&1 || fail "cần curl trên PATH"
command -v sha256sum >/dev/null 2>&1 || fail "cần sha256sum trên PATH"

VERSION="$(python3 -c 'import json; print(json.load(open("package.json"))["version"])')"
[ -n "$VERSION" ] || fail "không đọc được phiên bản trong package.json"
TAG="v$VERSION"
BASE="https://github.com/tamkogay9876-code/camareLOL/releases/download/$TAG"

echo "Đóng gói @wenker-ai/camarelo $VERSION ($TAG)"

# 1. tải SHA256SUMS + binary vào thư mục tạm (không đụng vendor cũ khi chưa xong)
TMP="$(mktemp -d /tmp/opencode/npm-build-XXXXXX)"
trap 'rm -rf "$TMP"' EXIT

curl -fsSL --max-time 120 -o "$TMP/SHA256SUMS" "$BASE/SHA256SUMS" \
  || fail "tải $BASE/SHA256SUMS thất bại — đúng bản v$VERSION đã phát hành chưa?"

EXPECTED="$(awk '$2 == "camarelol" || $2 == "*camarelol" { print $1 }' "$TMP/SHA256SUMS")"
[ -n "$EXPECTED" ] || fail "SHA256SUMS của $TAG không có dòng cho camarelol"

curl -fsSL --max-time 300 -o "$TMP/camarelol" "$BASE/camarelol" \
  || fail "tải $BASE/camarelol thất bại"

ACTUAL="$(sha256sum "$TMP/camarelol" | awk '{print $1}')"
if [ "$ACTUAL" != "$EXPECTED" ]; then
  fail "SHA-256 không khớp:
    tải về  : $ACTUAL
    khai báo: $EXPECTED
  Dừng. Không pack tệp chưa kiểm."
fi
echo "  SHA-256 khớp ($ACTUAL)"

# 2. binary phải tự báo đúng phiên bản — không chỉ khớp hash mà còn phải chạy
mkdir -p vendor
cp "$TMP/camarelol" vendor/camarelol
chmod 755 vendor/camarelol
OUT="$(./vendor/camarelol --version 2>&1)" || fail "binary tải về không chạy được: $OUT"
case "$OUT" in
  *" $VERSION "*) : ;;
  *) fail "binary tự báo \"$OUT\", không phải $VERSION" ;;
esac
echo "  binary chạy và tự báo đúng bản"

# 3. test rồi pack
node test/run.js || fail "test gói thất bại"
chmod 755 bin/camarelol.js vendor/camarelol
rm -f *.tgz
TARBALL="$(npm pack --pack-destination . 2>/dev/null | tail -1)"
[ -f "$TARBALL" ] || fail "npm pack không tạo ra tarball"
SRC="$TARBALL" python3 - <<'PY' || fail "python không chạy được"
import json, os, tarfile, sys
tar = tarfile.open(os.environ['SRC'])
names = {m.name for m in tar.getmembers()}
props = json.load(open('package.json'))
# npm pack đặt mọi tệp dưới thư mục gốc 'package/' bên trong tarball,
# bất kể tên gói là gì.
prefix = 'package/'
need = ['package.json', 'README.md', 'LICENSE'] + props['files']
missing = [f for f in need if not any(n.startswith(prefix + f.rstrip('/')) for n in names)]
if missing:
    print('  lỗi: tarball thiếu: %s' % ', '.join(missing), file=sys.stderr)
    sys.exit(1)
bin_member = prefix + props['bin']['camarelol']
if bin_member not in names:
    print('  lỗi: tarball thiếu %s' % bin_member, file=sys.stderr)
    sys.exit(1)
mode = tar.getmember(bin_member).mode
if not (mode & 0o111):
    print('  lỗi: %s trong tarball không có quyền thực thi' % bin_member, file=sys.stderr)
    sys.exit(1)
print('  tarball: %s — %d tệp, bin có quyền thực thi' % (os.environ['SRC'], len(names)))
PY

echo "Xong: npm/$TARBALL"

# 4. publish nếu được yêu cầu
if [ "${1:-}" = "publish" ]; then
  NPMRC="${2:-}"
  if [ -n "$NPMRC" ]; then
    [ -f "$NPMRC" ] || fail "không thấy tệp npmrc: $NPMRC"
    npm publish --access public --userconfig "$NPMRC" || fail "npm publish thất bại"
  else
    echo "  không có tệp npmrc được chỉ định — dùng ~/.npmrc nếu nó có token"
    npm publish --access public || fail "npm publish thất bại"
  fi
fi
