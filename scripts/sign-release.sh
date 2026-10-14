#!/usr/bin/env bash
#
# Ký chỉ mục cập nhật ngay trên máy của bạn, không cần GitHub secret.
#
#   ./scripts/sign-release.sh 0.1.9
#
# Nó tải latest.json của bản phát hành về, ký bằng khoá riêng trong
# ~/.gnupg, rồi tự kiểm tra lại bằng khoá công khai trong chính repo — trước
# khi báo xong. Tệp chữ ký nằm cạnh latest.json để bạn tải lên release.
#
# Vì sao đường này tốt hơn secret trên GitHub: khoá riêng không bao giờ rời
# khỏi máy bạn. Secret trên GitHub phải được sao chép qua trình duyệt, và một
# lần dán hụt thì bản phát hành đi ra không ký mà trông vẫn bình thường.
#
# Sau khi ký, tải latest.json.sig lên release. camarelol --check-update sẽ báo
# "signed by a pinned key" ngay.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION="${1:-}"
[ -n "$VERSION" ] || die "cách dùng: $0 <phiên bản, ví dụ 0.1.9>"
VERSION="${VERSION#v}"

OWNER="${CAMARELOL_OWNER:-tamkogay9876-code}"
NAME="${CAMARELOL_REPO:-camareLOL}"
OUT_DIR="${CAMARELOL_SIGN_OUT:-$REPO_DIR/out}"
BASE="https://github.com/$OWNER/$NAME/releases/download/v$VERSION"

die() { printf '%s\n' "$*" >&2; exit 1; }
note() { printf '  %s\n' "$*"; }

command -v gpg >/dev/null 2>&1 || die "không tìm thấy gpg"
command -v curl >/dev/null 2>&1 || die "không tìm thấy curl"

# --------------------------------------------------------- khoá cần dùng
# Đúng khoá mà repo đã ghim, lấy từ keys/camarelol.pub. Ký bằng khoá khác thì
# mọi máy đều từ chối bản cập nhật, nên kiểm tra từ đầu.
PINNED="$REPO_DIR/keys/camarelol.pub"
[ -f "$PINNED" ] || die "thiếu $PINNED"
FPR="$(gpg --batch --with-colons --import-options show-only --import "$PINNED" 2>/dev/null | awk -F: '/^fpr/{print $10; exit}')"
[ -n "$FPR" ] || die "đọc không được vân tay trong $PINNED"

if ! gpg --batch --list-secret-keys "$FPR" >/dev/null 2>&1; then
    die "bạn không có khoá bí mật $FPR trong keyring.
    Nếu đây là khoá mới, hãy nhập khoá riêng trước:
        gpg --import ~/camarelol-signing.asc"
fi

mkdir -p "$OUT_DIR"
INDEX="$OUT_DIR/latest.json"
SIG="$OUT_DIR/latest.json.sig"

printf '\nĐang tải chỉ mục của v%s...\n' "$VERSION"
curl --silent --show-error --fail --location --proto '=https' --proto-redir '=https' \
    --max-time 60 -o "$INDEX" "$BASE/latest.json" \
    || die "không tải được $BASE/latest.json
    Bản phát hành phải tồn tại trước khi ký. Nếu CI chưa phát hành vì thiếu
    khoá, hãy bật biến ALLOW_UNSIGNED_RELEASE trong repo rồi tag lại."

grep -q '"sha256"' "$INDEX" || die "file tải về không phải một chỉ mục cập nhật"

printf 'Đang ký bằng khoá %s...\n' "${FPR: -16}"
if gpg --batch --quiet --pinentry-mode loopback --passphrase "${CAMARELOL_SIGNING_PASSPHRASE:-}" \
        --local-user "$FPR" --detach-sign --armor -o "$SIG" "$INDEX" 2>/dev/null; then
    :
else
    # Không có mật khẩu đặt sẵn thì thử lại không dùng pinentry.
    gpg --batch --quiet --local-user "$FPR" --detach-sign --armor -o "$SIG" "$INDEX"
fi

# -------------------------------------------------- kiểm tra bằng chính khoá ghim
if ! gpg --batch --quiet --status-fd 1 --verify "$SIG" "$INDEX" 2>/dev/null \
        | grep -q "VALIDSIG $FPR"; then
    die "chữ ký không khớp khoá đã ghim — KHÔNG tải file này lên release"
fi

printf '\nXong, và đã tự kiểm tra lại bằng khoá công khai trong repo.\n\n'
note "chỉ mục: $INDEX"
note "chữ ký: $SIG"
printf '\nCòn một việc: tải chữ ký lên release v%s\n\n' "$VERSION"
cat <<EOF
    gh release upload v$VERSION "$SIG" --clobber

Nếu không có gh, dùng trang web:
    https://github.com/$OWNER/$NAME/releases/tag/v$VERSION
    → bấm "Edit release" → kéo thả $SIG vào khung tệp đính kèm

Sau đó kiểm tra:

    camarelol --check-update --update-url $BASE/latest.json

Phải in ra "signed by a pinned key".
EOF