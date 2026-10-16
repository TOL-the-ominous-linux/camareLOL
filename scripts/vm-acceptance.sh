#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# camareLOL — VirtualBox acceptance test that can actually fail.
#
#   ./scripts/vm-acceptance.sh            # chạy đầy đủ
#   ./scripts/vm-acceptance.sh --keep     # giữ VM chạy để soi tay
#
# ---------------------------------------------------------------------------
# Vì sao script này tồn tại
#
# vm-test.sh và vm-release-test.sh chụp ảnh màn hình rồi coi như xong:
# `cmd_check` của vm-test.sh nói thẳng "the last screenshot is the source of
# truth" nhưng không có chỗ nào đọc ảnh — không OCR, không đối chiếu. `verify`
# của vm-release-test.sh cũng vậy: chụp một tấm, in đường dẫn, exit 0.
#
# Bài kiểm tra không thể thất bại không kiểm tra gì. Nó đã báo xanh trong khi
# dòng 84 của vm-release-test.sh gõ `camarelol --self-test` — thiếu `/root/`,
# nên lệnh đó là "command not found" và không ai thấy.
#
# Ở đây khách gửi kết quả về máy chủ qua NAT (10.0.2.2 là máy chủ), không cần
# Guest Additions. Mỗi bước POST ngay khi xong, nên bước treo không nuốt kết
# quả của những bước trước — và ta biết nó treo ở đâu thay vì "không có gì".
# ---------------------------------------------------------------------------
set -uo pipefail

VM_NAME="camarelol-acceptance"
VM_DIR="${HOME}/VirtualBox VMs/${VM_NAME}"
ISO="${ISO:-$HOME/Tài liệu/carameLOL/.test/archlinux-x86_64.iso}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PORT="${PORT:-8899}"
RESULTS="${RESULTS:-/tmp/opencode/vm-accept}"
SERVE="${SERVE:-/tmp/opencode/vm-serve}"
HOST_ADDR="10.0.2.2"

KEEP=0
[ "${1:-}" = "--keep" ] && KEEP=1

FAILURES=0
SERVER_PID=""

say()  { printf '\n=== %s ===\n' "$*"; }
note() { printf '  %s\n' "$*"; }
vm()   { VBoxManage "$@" 2>/dev/null; }

# Gõ một lệnh vào khách rồi chờ. Enter phải nằm trong chuỗi: `keyboardputscancode
# Enter` không bao giờ tới được shell khách (đã ghi trong vm-release-test.sh).
guest() {
    vm controlvm "$VM_NAME" keyboardputstring "$1
" >/dev/null
    sleep "${2:-5}"
}

post() { # post <tên>  — tệp kết quả đã nằm ở /tmp/r-<tên>.txt trong khách
    guest "curl -sS -X POST --data-binary @/tmp/r-$1.txt http://$HOST_ADDR:$PORT/step/$1 >/dev/null 2>&1; echo done-$1" 4
}

# Đọc một kết quả từ máy chủ.
result() { cat "$RESULTS/step_$1.txt" 2>/dev/null || echo ""; }

fail() { FAILURES=$((FAILURES + 1)); printf '  \033[31mFAIL\033[0m %s\n' "$*"; }
pass() { printf '  \033[32mPASS\033[0m %s\n' "$*"; }

# ------------------------------------------------------- dọn dẹp trước
cleanup() {
    [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null
    if [ "$KEEP" = 1 ]; then
        note "giữ VM chạy (--keep). Tắt bằng: VBoxManage controlvm $VM_NAME poweroff"
    else
        vm controlvm "$VM_NAME" poweroff >/dev/null 2>&1
    fi
}
trap cleanup EXIT

say "dọn kết quả cũ"
rm -rf "$RESULTS" "$SERVE"
mkdir -p "$RESULTS" "$SERVE"
# Chép đúng những gì khách sẽ dùng. Không dùng bản trong kho: bài kiểm tra này
# kiểm tra *thứ đã phát hành*, và chỉ máy chủ mới biết thứ đó là gì.
cp "$ROOT/out/release/camarelol" "$SERVE/camarelol"
cp "$ROOT/scripts/vm-helper.py" "$SERVE/vm-helper.py"

say "khởi động máy chủ kết quả (cổng $PORT)"
python3 "$ROOT/scripts/vm-result-server.py" --port "$PORT" --out "$RESULTS" \
        --serve-dir "$SERVE" > "$RESULTS/server.log" 2>&1 &
SERVER_PID=$!
sleep 2
if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    say "máy chủ kết quả không lên:"; cat "$RESULTS/server.log"; exit 1
fi
cat "$RESULTS/server.log" | sed 's/^/  /'

say "tạo VM"
if vm list vms | grep -q "\"$VM_NAME\""; then
    vm controlvm "$VM_NAME" poweroff >/dev/null 2>&1; sleep 2
    vm unregistervm "$VM_NAME" --delete >/dev/null 2>&1
fi
vm createvm --name "$VM_NAME" --ostype ArchLinux_64 --register
vm modifyvm "$VM_NAME" --memory 4096 --cpus 4 --vram 16 \
    --boot1 dvd --boot2 disk --audio-driver none --usb off \
    --clipboard-mode disabled
# Khách phải tới được dịch vụ trên loopback của máy chủ. Không bật thì mọi
# `curl http://10.0.2.2:PORT` bị từ chối, và bài kiểm tra im lặng thất bại ở
# đúng chỗ đáng lẽ phải kiểm tra binary.
#
# VirtualBox không có cờ dòng lệnh cho nó: nó nằm trong tệp .vbox dưới dạng
# `<NAT localhost-reachable="true"/>`, và mặc định là tắt. VM `camarelol-release-
# test` có nó bật sẵn nên lần chạy đầu của script này — trên một VM mới — thất
# bại im lặng, đúng những gì nó sinh ra để chống lại.
VM_VBOX="${VM_DIR}/${VM_NAME}.vbox"
python3 - "$VM_VBOX" <<'PY'
import pathlib, sys
p = pathlib.Path(sys.argv[1])
s = p.read_text()
if "<NAT/>" in s:
    p.write_text(s.replace("<NAT/>", '<NAT localhost-reachable="true"/>', 1))
    print("  bật localhost-reachable cho NAT")
elif 'localhost-reachable="true"' in s:
    print("  NAT đã cho phép localhost")
else:
    print("  CẢNH BÁO: không tìm thấy khối <NAT> trong .vbox — kiểm tra thủ công")
PY
# Hai đĩa trống: một cho smoke test, một cho bản cài thật.
vm storagectl "$VM_NAME" --name SATA --add sata --controller IntelAhci
for port in 0 1; do
    vm createmedium disk --filename "${VM_DIR}/disk${port}.vdi" --size 16384 --format VDI
    vm storageattach "$VM_NAME" --storagectl SATA --port "$port" --device 0 \
        --type hdd --medium "${VM_DIR}/disk${port}.vdi"
done
vm storagectl "$VM_NAME" --name IDE --add ide
vm storageattach "$VM_NAME" --storagectl IDE --port 0 --device 0 \
    --type dvddrive --medium "$ISO"
note "Arch ISO + 2 đĩa trống 16 GiB"

say "khởi động VM (chờ 100s cho môi trường live)"
vm startvm "$VM_NAME" --type headless
sleep 100
guest "" 5   # Enter để đảm bảo có prompt

# ------------------------------------------------------------- các bước

say "bước 1/5 — tải binary từ máy chủ"
guest "curl -sS -o /root/camarelol http://$HOST_ADDR:$PORT/camarelol > /tmp/r-download.txt 2>&1 && chmod +x /root/camarelol && echo downloaded >> /tmp/r-download.txt; /root/camarelol --version >> /tmp/r-download.txt 2>&1; echo rc=\$? >> /tmp/r-download.txt" 25
post download

say "bước 2/5 — quét thiết bị thật (--scan)"
guest "/root/camarelol --scan > /tmp/survey.json 2>/tmp/scan.err; echo rc=\$? > /tmp/r-scan.txt; echo bytes=\$(stat -c%s /tmp/survey.json) >> /tmp/r-scan.txt; cat /tmp/scan.err >> /tmp/r-scan.txt; grep -c path /tmp/survey.json >> /tmp/r-scan.txt" 20
post scan

say "bước 3/5 — self-test"
guest "/root/camarelol --self-test > /tmp/st.txt 2>&1; echo rc=\$? > /tmp/r-selftest.txt; cat /tmp/st.txt >> /tmp/r-selftest.txt" 25
post selftest

say "bước 4/5 — --help không còn cờ cập nhật"
guest "/root/camarelol --help > /tmp/help.txt 2>&1; echo rc=\$? > /tmp/r-help.txt; grep -cE '^    --(check-)?update|rollback-update' /tmp/help.txt >> /tmp/r-help.txt; grep -A3 'UPDATING' /tmp/help.txt >> /tmp/r-help.txt" 12
post help

say "bước 5/5 — cài thật vào đĩa trống (--scripted --execute)"
guest "curl -sS -o /root/vm-helper.py http://$HOST_ADDR:$PORT/vm-helper.py >/dev/null 2>&1; python3 /root/vm-helper.py /tmp/survey.json /tmp/profile.toml > /tmp/r-install.txt 2>&1; echo profile_rc=\$? >> /tmp/r-install.txt" 15
post install_profile
note "chạy cài đặt — có thể mất vài phút"
guest "/root/camarelol --scripted /tmp/profile.toml --allow-destructive --execute > /tmp/inst.txt 2>&1; echo INSTALL_RC=\$? > /tmp/r-install.txt; cat /tmp/inst.txt >> /tmp/r-install.txt" 60
post install

say "đọc kết quả và kết luận"
python3 "$ROOT/scripts/vm-check-results.py" --results "$RESULTS" --expect-version 0.1.11
CHECK_RC=$?

VBoxManage controlvm "$VM_NAME" screenshotpng "$RESULTS/99-final.png" >/dev/null 2>&1 || true
note "ảnh cuối: $RESULTS/99-final.png"
note "kết quả thô:  $RESULTS/step_*.txt"

if [ "$CHECK_RC" -ne 0 ] || [ "$FAILURES" -ne 0 ]; then
    printf '\n\033[31m=== THẤT BẠI ===\033[0m\n'
    exit 1
fi
printf '\n\033[32m=== ĐẠT ===\033[0m\n'
exit 0
