#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# camareLOL — cài binary bằng một dòng, chạy được trực tiếp từ live ISO.
#
#   curl -fsSL https://raw.githubusercontent.com/tamkogay9876-code/camareLOL/main/scripts/camarelol.sh | bash
#
# Tệp này tự chứa, không cần clone kho. Nó làm đúng ba việc rồi đưa quyền
# kiểm soát lại cho bạn: tải binary, CHỨNG MINH nó là binary thật, và chạy
# --self-test (không ghi đĩa nào).
#
# Vì sao phải kiểm tra thế?
#
#   $ curl -sSLO https://github.com/.../camarelol      # thiếu -f và -L
#   $ chmod +x camarelol
#   $ ./camarelol --self-test
#   ./camarelol: line 1: Not: command not found
#
# Câu đó không phải lỗi của camareLOL. `curl -sSLO` không thất bại khi HTTP
# trả về trang lỗi, nên nó lưu trang lỗi (văn bản "Not Found") xuống đúng
# tên file `camarelol`. chmod +x thì vẫn thành công — thứ không phải ELF mà
# là kịch bản shell, nên bash cố chạy nó và báo dòng 1 không có lệnh.
#
# Một installer xoá ổ đĩa không được phép bắt người dùng tin rằng "đã tải
# xong" khi thứ đã tải về là HTML. Dưới đây là ba lớp kiểm: HTTP, ELF magic,
# SHA-256 — theo đúng thứ tự đó.
#
# Biến môi trường:
#   CAMARELOL_PREFIX   nơi cài (mặc định /usr/local/bin)
#   CAMARELOL_VERSION  ghim một bản cụ thể, ví dụ v0.1.15 (mặc định: mới nhất)
#   CAMARELOL_NOINSTALL=1  không cài vào hệ thống, chỉ tải về thư mục hiện tại
# ---------------------------------------------------------------------------
set -euo pipefail

REPO="tamkogay9876-code/camareLOL"
API="https://api.github.com/repos/$REPO/releases/latest"
PREFIX="${CAMARELOL_PREFIX:-/usr/local/bin}"
NOINSTALL="${CAMARELOL_NOINSTALL:-0}"

info() { printf '\033[1;36m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m!!\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31mxx\033[0m %s\n' "$*" >&2; exit 1; }

need() { command -v "$1" >/dev/null 2>&1 || die "cần $1 nhưng không thấy trên PATH"; }
need curl
need sha256sum

case "$(uname -m)" in
    x86_64|amd64) ;;
    *) die "camareLOL chỉ có binary cho x86_64; máy này là $(uname -m)" ;;
esac

# -------------------------------------------------------------- bản phát hành
resolve_tag() {
    if [ -n "${CAMARELOL_VERSION:-}" ]; then
        case "$CAMARELOL_VERSION" in
            v*) printf '%s' "$CAMARELOL_VERSION" ;;
            *)  printf 'v%s' "$CAMARELOL_VERSION" ;;
        esac
        return
    fi
    # tag_name nằm ở đầu payload, nên grep là đủ; không cần phân tích JSON.
    curl -fsSL --retry 2 --max-time 30 "$API" \
        | grep -m1 '"tag_name"' \
        | sed 's/.*"tag_name"[[:space:]]*:[[:space:]]*"//; s/".*//' \
        || die "không đọc được bản mới nhất từ GitHub"
}

TAG="$(resolve_tag)"
[ -n "$TAG" ] || die "không xác định được tag phát hành"
BASE="https://github.com/$REPO/releases/download/$TAG"
info "bản phát hành: $TAG"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# ------------------------------------------------------------------- tải về
# -f: HTTP lỗi thì thất bại, không lưu trang lỗi xuống đĩa.
# -L: theo redirect sang CDN.
info "tải camarelol"
curl -fsSL --retry 3 --max-time 300 "$BASE/camarelol" -o "$TMP/camarelol" \
    || die "tải $BASE/camarelol thất bại — bản $TAG chưa phát hành, hoặc mạng bị chặn"

info "tải SHA256SUMS"
curl -fsSL --retry 3 --max-time 60 "$BASE/SHA256SUMS" -o "$TMP/SHA256SUMS" \
    || die "tải $BASE/SHA256SUMS thất bại"

# ------------------------------------------------ lớp 1: đây có phải ELF không
# Không phụ thuộc `file` (ISO live không phải lúc nào cũng có), chỉ đọc 4
# byte đầu. \x7fELF là magic number của một executable Linux.
info "kiểm tra định dạng"
MAGIC="$(od -An -tx1 -N4 "$TMP/camarelol" 2>/dev/null | tr -d ' \n')"
case "$MAGIC" in
    7f454c46) : ;;   # \x7fELF
    "") die "tệp tải về rỗng — mạng bị chặn hoặc CDN trả rỗng" ;;
    *) die "tệp tải về KHÔNG phải binary Linux (magic=$MAGIC).
   Rất có thể bạn gặp đúng lỗi này: URL sai, hoặc một proxy/captive portal
   trả về trang lỗi thay vì tệp. Kiểm tra bằng tay:
     curl -fL -o camarelol $BASE/camarelol && file camarelol" ;;
esac

# ------------------------------------------------------- lớp 2: SHA-256 thật
info "kiểm tra SHA-256"
EXPECTED="$(awk '$2 == "camarelol" || $2 == "*camarelol" { print $1 }' "$TMP/SHA256SUMS")"
[ -n "$EXPECTED" ] || die "SHA256SUMS của $TAG không có dòng cho camarelol"
ACTUAL="$(sha256sum "$TMP/camarelol" | awk '{print $1}')"
if [ "$ACTUAL" != "$EXPECTED" ]; then
    die "SHA-256 không khớp:
   tải về  : $ACTUAL
   khai báo: $EXPECTED
   Dừng. Một tệp đã đổi nội dung giữa đường không được chạy."
fi
info "SHA-256 khớp ($ACTUAL)"

# ------------------------------------------- lớp 3: binary có tự chạy không
chmod +x "$TMP/camarelol"
VERSION="$("$TMP/camarelol" --version 2>&1)" || die "binary tải về không chạy được:
$VERSION"
case "$VERSION" in
    *"$TAG"*) ;;
    *) die "binary tự báo \"$VERSION\", không khớp $TAG" ;;
esac
info "binary chạy và tự báo đúng: $VERSION"

# ------------------------------------------------------------------- cài đặt
if [ "$NOINSTALL" = "1" ]; then
    TARGET="./camarelol"
else
    need install
    if [ "$(id -u)" -eq 0 ]; then
        install -Dm755 "$TMP/camarelol" "$PREFIX/camarelol"
    elif command -v sudo >/dev/null 2>&1; then
        info "cần quyền root để cài vào $PREFIX — hỏi mật khẩu"
        sudo install -Dm755 "$TMP/camarelol" "$PREFIX/camarelol"
    else
        warn "không có sudo; cài vào ./camarelol thay vì $PREFIX"
        TARGET="./camarelol"
    fi
    TARGET="$PREFIX/camarelol"
fi
[ -n "${TARGET:-}" ] || TARGET="$PREFIX/camarelol"
install -m755 "$TMP/camarelol" "$TARGET" 2>/dev/null || cp "$TMP/camarelol" "$TARGET"
chmod +x "$TARGET"
info "đã cài: $TARGET"

# -------------------------------------------------------------- tự kiểm tra
info "chạy --self-test (không ghi đĩa nào)"
"$TARGET" --self-test

cat <<'EOF'

--------------------------------------------------------------------------
Đã sẵn sàng. Hai bước tiếp theo, theo đúng thứ tự:

  1. Chạy thử không phá gì — chỉ xem:
       camarelol --self-test
       camarelol --scan

  2. Cài thật, trên Arch live ISO, với cờ an toàn bắt buộc:
       sudo camarelol --allow-destructive

     `--allow-destructive` mới MỞ safety gate. Chưa có nó thì installer
     chỉ ghi vào RAM (dry-run) — không xoá gì cả. Màn Review vẫn bắt
     bạn gõ đúng cụm xác nhận (ERASE hoặc ERASE AND WIPE) trước khi
     chạm vào ổ đĩa.

Muốn cài không cần giao diện:
       sudo camarelol --scripted <profile.toml> --allow-destructive --execute

camareLOL không tự cập nhật. Cài bản khác: chạy lại tệp này với
CAMARELOL_VERSION, hoặc tải thẳng từ trang Releases. Gỡ thì chỉ cần
xoá đúng một tệp:
       sudo rm -f $PREFIX/camarelol
--------------------------------------------------------------------------
EOF