#!/usr/bin/env bash
#
# Gom tài nguyên cho một bản phát hành. Không tự tạo release, không ký.
#
#   ./scripts/make-release.sh 0.1.10
#
# Đưa ra out/:
#   camarelol                 binary, build từ thẻ đúng phiên bản này
#   camarelol-X.Y.Z.tar.gz    mã nguồn, để PKGBUILD dựng lại
#   camarelol-catalog.jar     catalog Java
#   latest.json               chỉ mục cập nhật, kèm sha256 của binary
#
# Rồi tạo release trên GitHub, chạy scripts/sign-release.sh, và tải
# latest.json.sig lên. Ba bước đó cố ý không nằm ở đây: bước đầu cần mạng,
# bước sau cần khoá bí mật, và gộp chúng vào một script sẽ khiến việc "chỉ
# build" trở thành việc "phát hành thật" chỉ vì gõ sai một tham số.
#
# Chỉ dùng trên máy của bạn. Không chạy trên máy dùng thật — xoá ổ đĩa không
# phải thứ để thử ở đây.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_DIR"

VERSION="${1:-}"
[ -n "$VERSION" ] || { printf '%s\n' "cách dùng: $0 <phiên bản, ví dụ 0.1.10>" >&2; exit 1; }
VERSION="${VERSION#v}"

OUT_DIR="${CAMARELOL_RELEASE_OUT:-$REPO_DIR/out/release}"
BASE_URL="${CAMARELOL_BASE_URL:-https://github.com/tamkogay9876-code/camareLOL/releases/download/v$VERSION}"

die() { printf '%s\n' "$*" >&2; exit 1; }
note() { printf '  %s\n' "$*"; }

command -v cargo >/dev/null 2>&1 || die "không tìm thấy cargo"
command -v tar >/dev/null 2>&1 || die "không tìm thấy tar"

# ------------------------------------------------------- thẻ phải là thẻ này
# Binary được build từ thẻ đang được phát hành. Build từ một commit khác
# rồi gắn tên bản phát hành khác lên nghĩa là có hai camareLOL khác nhau cùng
# mang tên một bản phát hành, và không có gì sau đó phân biệt được chúng.
TAG="v$VERSION"
git rev-parse --verify --quiet "refs/tags/$TAG" >/dev/null \
    || die "chưa có thẻ $TAG.
    Đẩy mã nguồn trước, rồi đặt thẻ:
        git tag -a $TAG -m 'camareLOL $VERSION'
        git push origin $TAG"

if ! git diff --quiet || ! git diff --cached --quiet; then
    die "cây làm việc bẩn — build từ trạng thái không ai biết.
    Commit hoặc stash trước."
fi

# --------------------------------------------------- đúng thẻ, không phải HEAD
# `git checkout` vào thẻ rồi build, để binary mang đúng mã nguồn được gắn tên.
# Trả về nhánh cũ về cuối, kể cả khi build hỏng.
ORIGINAL_REF="$(git symbolic-ref --quiet --short HEAD || git rev-parse HEAD)"
restore() {
    if [ "$ORIGINAL_REF" = "$(git symbolic-ref --quiet --short HEAD || echo HEAD)" ] \
       || git rev-parse --verify --quiet "$ORIGINAL_REF" >/dev/null; then
        git checkout --quiet "$ORIGINAL_REF" 2>/dev/null || true
    fi
}
trap restore EXIT

git checkout --quiet "$TAG"
note "đang build từ $TAG ($(git rev-parse --short HEAD))"

# ------------------------------------------------------------------ binary
# -j 2: máy này từng làm nghẽn với mức song song mặc định.
cargo build --release -j 2 -p camarelol
mkdir -p "$OUT_DIR"
install -m 0755 target/release/camarelol "$OUT_DIR/camarelol"

# ------------------------------------------------------------- catalog Java
# Không phải thứ binary cần, nhưng release nào cũng kèm, và dựng lại từ mã
# nguồn thay vì lấy từ máy nào đó.
if command -v javac >/dev/null 2>&1; then
    java/build.sh "$OUT_DIR/java" >/dev/null
    cp "$OUT_DIR/java/camarelol-catalog.jar" "$OUT_DIR/camarelol-catalog.jar"
    note "catalog Java"
else
    note "bỏ qua catalog Java — không tìm thấy javac"
fi

# --------------------------------------------------------- tarball mã nguồn
TARBALL="$OUT_DIR/camarelol-$VERSION.tar.gz"
STAGE="$OUT_DIR/.stage"
rm -rf "$STAGE"
mkdir -p "$STAGE/camarelol-$VERSION"
# git archive lấy đúng nội dung thẻ, không lấy thứ nằm trong thư mục làm việc.
git archive --format=tar "$TAG" | tar -x -C "$STAGE/camarelol-$VERSION"
tar -czf "$TARBALL" -C "$STAGE" "camarelol-$VERSION"
rm -rf "$STAGE"
note "tarball $(basename "$TARBALL")"

# ------------------------------------------------------------ latest.json
# Chỉ mục trỏ vào URL tải trên release, nên nó chỉ đúng sau khi release tồn
# tại. sha256 là của binary trong chính release này — số ở đây phải khớp với
# số tải về, nếu không client sẽ từ chối bản cập nhật.
SHA="$(sha256sum "$OUT_DIR/camarelol" | cut -d' ' -f1)"
cat > "$OUT_DIR/latest.json" <<EOF
{
  "schema": 1,
  "version": "$VERSION",
  "url": "$BASE_URL/camarelol",
  "sha256": "$SHA"
}
EOF

# Chỉ mục phải tự mô tả được: số trong nó phải là số của chính binary này.
grep -q "$SHA" "$OUT_DIR/latest.json" || die "chỉ mục không khớp sha256"

# ------------------------------------------------ binary có đúng thẻ không
# Cargo có thể dựng lại mà không chạy lại build script, khiến nó trả về binary
# của lần build trước — binary đó vẫn mang thẻ cũ. Thẻ trong tên tệp là ý
# định; thẻ bên trong binary là sự thật, và hai thứ phải khớp trước khi phát
# hành. Đây là chốt cuối, không phải kiểm tra cho đẹp: nó bắt được đúng lỗi khiến
# một bản phát hành mang tên 0.1.10 nhưng chạy code của 0.1.9.
BUILT="$("$OUT_DIR/camarelol" --version | head -1)"
case "$BUILT" in
    *"tag $TAG"*) : ;;
    *) die "binary vừa dựng không tự nhận là $TAG:
    $BUILT
    Nó là binary của lần build trước. Xoá target/ rồi dựng lại:
        rm -rf target && $0 $VERSION" ;;
esac
grep -q "dirty working tree" <<<"$BUILT" \
    && die "binary tự báo cây làm việc bẩn:
    $BUILT"

printf '\nXong. Kiểm tra lại trước khi phát hành:\n\n'
note "phiên bản trong binary : $("$OUT_DIR/camarelol" --version | head -1)"
note "sha256                : $SHA"
note "kênh cập nhật         : $("$OUT_DIR/camarelol" --check-update 2>&1 | grep -o 'index: .*' | head -1)"
printf '\nTiếp theo:\n\n'
note "1. gh release create $TAG --title ... --notes ... $OUT_DIR/camarelol"
note "2. $OUT_DIR/latest.json  $OUT_DIR/camarelol-$VERSION.tar.gz  $OUT_DIR/camarelol-catalog.jar"
note "3. ./scripts/sign-release.sh $VERSION"
note "4. gh release upload $TAG $OUT_DIR/latest.json $OUT_DIR/latest.json.sig"