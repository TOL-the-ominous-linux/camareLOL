#!/usr/bin/env bash
#
# Tạo cặp khoá ký cho chỉ mục cập nhật, không cần bấm tay từng bước.
#
#   ./scripts/make-signing-key.sh
#
# Nó làm ba việc:
#   1. tạo khoá GPG (Ed25519, chỉ ký, không hết hạn)
#   2. ghi khoá công khai vào keys/camarelol.pub — file này COMMIT vào kho mã
#      nguồn, vì nó được nhúng vào binary lúc build
#   3. ghi khoá riêng vào ~/camarelol-signing.asc — file này KHÔNG commit;
#      bạn tự dán nội dung của nó vào GitHub secret CAMARELOL_SIGNING_KEY
#
# Khoá riêng không bao giờ được in ra màn hình và không bao giờ được commit.
# Nếu nó lọt vào bất kỳ đâu không phải GitHub secret, hãy coi khoá đó là đã hỏng
# và tạo lại từ đầu.
#
# Đặt mật khẩu (tuỳ chọn):
#   ./scripts/make-signing-key.sh --passphrase
# rồi tạo thêm secret CAMARELOL_SIGNING_PASSPHRASE trên GitHub.

set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
KEY_FILE="$REPO/keys/camarelol.pub"
SECRET_FILE="${CAMARELOL_SECRET_FILE:-$HOME/camarelol-signing.asc}"
KEY_NAME="${CAMARELOL_KEY_NAME:-camareLOL release}"
KEY_EMAIL="${CAMARELOL_KEY_EMAIL:-}"

PASSPHRASE=""
if [ "${1:-}" = "--passphrase" ]; then
    printf 'Mật khẩu cho khoá riêng: '
    read -r -s PASSPHRASE
    printf '\n'
fi

die() { printf '%s\n' "$*" >&2; exit 1; }
note() { printf '  %s\n' "$*"; }

command -v gpg >/dev/null 2>&1 || die "không tìm thấy gpg (cài gpg hoặc gnupg)"

if [ -z "$KEY_EMAIL" ]; then
    printf 'Email cho khoá (sẽ nằm trong khoá công khai, ai cũng thấy): '
    read -r KEY_EMAIL
fi
[ -n "$KEY_EMAIL" ] || die "cần email"

mkdir -p "$REPO/keys"

# ---------------------------------------------------------------- old keys
# Một khoá từng bị lộ thì phải bị xoá, không phải chỉ phớt lờ. Nhắc trước,
# không tự xoá: xoá khoá là việc không thể đảo ngược.
# Cần kiểm tra cả khoá công khai, không chỉ khoá bí mật: xoá khoá bí mật đi
# không xoá khoá công khai, và gpg vẫn từ chối tạo khoá mới trùng user-id
# ("A key for ... already exists"). Script cũ chỉ nhìn khoá bí mật nên bỏ lọt
# đúng trường hợp đó.
list_keys() {
    gpg --batch --list-keys --with-colons 2>/dev/null | awk -F: '/^pub/{print $5}'
}

existing="$(list_keys)"
if [ -n "$existing" ]; then
    printf '\nKeyring của bạn đã có khoá:\n'
    while read -r fpr; do
        [ -n "$fpr" ] && note "$fpr"
    done <<< "$existing"
    printf '\nNếu bất kỳ khoá nào từng bị dán ra nơi công khai, hãy xoá nó:\n'
    printf '    gpg --delete-keys <MÃ-KHOÁ>\n'
    printf '\n(Lệnh này xoá cả khoá công khai lẫn khoá bí mật. Chỉ dùng\n'
    printf ' --delete-secret-keys sẽ để lại khoá công khai và lần chạy sau\n'
    printf ' vẫn thất bại.)\n'
    printf 'Sau đó chạy lại script này.\n\n'
    read -r -p 'Đã xoá xong, tiếp tục? [y/N] ' answer
    case "$answer" in
        [yY]|[yY][eE][sS]) ;;
        *) die "đã dừng" ;;
    esac
    still="$(list_keys)"
    [ -z "$still" ] || die "keyring vẫn còn khoá — xoá hết rồi hãy chạy lại"
fi

# --------------------------------------------------------------- generate
printf '\nĐang tạo khoá...\n'
if [ -n "$PASSPHRASE" ]; then
    gpg --batch --quiet --pinentry-mode loopback --passphrase "$PASSPHRASE" \
        --quick-generate-key "$KEY_NAME <$KEY_EMAIL>" ed25519 sign never
else
    gpg --batch --quiet --passphrase '' \
        --quick-generate-key "$KEY_NAME <$KEY_EMAIL>" ed25519 sign never
fi

FPR="$(gpg --batch --list-secret-keys --with-colons "$KEY_EMAIL" | awk -F: '/^fpr/{print $10; exit}')"
[ -n "$FPR" ] || die "không tìm thấy khoá vừa tạo"

# ----------------------------------------------------------------- export
gpg --armor --export "$FPR" > "$KEY_FILE"
gpg --armor --export-secret-keys "$FPR" > "$SECRET_FILE"
chmod 600 "$SECRET_FILE"

# ----------------------------------------------------------------- verify
# Số vân tay phải khớp: khoá công khai commit lên GitHub và khoá riêng đi vào
# secret là hai thứ khác nhau, và nếu lệch thì mọi bản cập nhật sau này đều bị
# từ chối mà không rõ lý do.
exported="$(gpg --batch --with-colons --import-options show-only --import "$KEY_FILE" 2>/dev/null | awk -F: '/^fpr/{print $10; exit}')"
[ "$exported" = "$FPR" ] || die "khoá xuất ra không khớp với khoá vừa tạo"

probe="$(mktemp)"
printf 'camareLOL signing key check\n' > "$probe"
gpg --batch --quiet --pinentry-mode loopback --passphrase "$PASSPHRASE" \
    --detach-sign --local-user "$FPR" -o "$probe.sig" "$probe" 2>/dev/null
gpg --batch --quiet --verify "$probe.sig" "$probe" >/dev/null 2>&1 \
    || die "ký thử không thành công"
rm -f "$probe" "$probe.sig"

printf '\nXong.\n\n'
printf '  vân tay khoá:  %s\n' "$FPR"
printf '  khoá công khai: %s\n' "$KEY_FILE"
printf '  khoá riêng:     %s (%s byte, quyền 600)\n\n' "$SECRET_FILE" "$(wc -c < "$SECRET_FILE")"

cat <<EOF
Còn hai việc:

1. Đưa khoá công khai lên GitHub

       git add keys/camarelol.pub
       git commit -m "add update signing key"
       git push

2. Tạo secret trên GitHub: Settings > Secrets and variables > Actions
   > New repository secret

       CAMARELOL_SIGNING_KEY  =  nội dung của $SECRET_FILE

   Mở file đó bằng "xdg-open $SECRET_FILE" rồi copy nguyên văn, KHÔNG chép tay:
   thiếu dấu gạch hoặc ngắt dòng sai chỗ thì GPG không nhập được.
EOF

if [ -n "$PASSPHRASE" ]; then
    cat <<EOF

   Khoá của bạn có mật khẩu, nên thêm secret nữa:

       CAMARELOL_SIGNING_PASSPHRASE  =  mật khẩu đó
EOF
fi