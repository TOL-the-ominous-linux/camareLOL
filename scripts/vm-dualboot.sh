#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# camareLOL — bằng chứng VM cho dual-boot (giữ phân vùng, dùng lại ESP).
#
#   ./scripts/vm-dualboot.sh          # chạy đầy đủ
#
# Bài kiểm tra này CHỈ có nghĩa nếu nó thật sự có thể thất bại:
#   - "Windows" giả được tạo TRƯỚC bằng sfdisk + mkfs.fat, kèm TỆP ĐÁNH DẤU
#     trên ESP, C:, recovery — nếu đường cài chạm vào chúng → marker biến
#     mất → FAIL.
#   - Cài qua chính camareLOL (--scripted --execute, reuse_esp = true).
#   - Verify trên live env: marker còn nguyên, EFI/camareLOL nằm trên ESP
#     cũ (sda1, KHÔNG format lại), fstab có /boot/efi,
#     GRUB_DISABLE_OS_PROBER=false, os-prober được cài.
#
# Kế thừa cấu trúc scripts/vm-acceptance.sh: NIC warm-up, `post` chờ tệp về
# thật thay vì ngủ cứng, NAT tới máy chủ trên máy thật (10.0.2.2).
# ---------------------------------------------------------------------------
set -uo pipefail

VM_NAME="${VM_NAME:-camarelol-dualboot}"
PORT="${PORT:-8898}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RESULTS="${RESULTS:-/tmp/camarelol-dualboot-results}"
SERVE="${SERVE:-/tmp/camarelol-dualboot-serve}"
VM_DIR="${VM_DIR:-$HOME/VirtualBox VMs/$VM_NAME}"
ISO="${CAMARELOL_ISO:-$ROOT/.test/archlinux-x86_64.iso}"
HOST_ADDR="10.0.2.2"
DISK_SIZE_MB=24576   # 24 GiB: "Windows" ~7 GiB + khe trống ~17 GiB

[ -f "$ISO" ] || { echo "  FAIL: không tìm thấy ISO: $ISO"; exit 1; }

FAILURES=0
SERVER_PID=""

say()  { printf '\n=== %s ===\n' "$*"; }
note() { printf '  %s\n' "$*"; }
vm()   { VBoxManage "$@" 2>/dev/null; }

guest() {
    vm controlvm "$VM_NAME" keyboardputstring "$1
" >/dev/null
    sleep "${2:-5}"
}

post() { # post <tên> [timeout_s] — chờ TỆP kết quả về thật sự
    guest "X=\$(curl -sS -m 8 -o /dev/null -w '%{http_code}' -X POST --data-binary @/tmp/r-$1.txt http://$HOST_ADDR:$PORT/step/$1 2>&1); echo post-$1 http=\$X; echo done-$1" 1
    local to="${2:-120}" waited=0
    while [ $waited -lt $to ]; do
        [ -e "$RESULTS/step_$1.txt" ] && return 0
        sleep 2
        waited=$((waited + 2))
    done
    note "cảnh báo: $RESULTS/step_$1.txt chưa về sau ${to}s"
}

# pass/fail + check <file> <chuỗi> <tên>
pass() { printf '  \033[32mPASS\033[0m %s\n' "$*"; }
fail() { FAILURES=$((FAILURES + 1)); printf '  \033[31mFAIL\033[0m %s\n' "$*"; }
check() {
    if grep -qF -- "$2" "$1"; then pass "$3"; else fail "$3 (thiếu '$2')"; fi
}
# check_kv <key>=<giá trịmong> <file> <tên>
check_kv() {
    if grep -qF -- "$1" "$2"; then pass "$1 — $3"; else fail "$1 — $3"; fi
}

cleanup() {
    [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null
    if [ "${KEEP:-0}" = 1 ]; then
        note "--keep: giữ VM. Tắt bằng: VBoxManage controlvm $VM_NAME poweroff"
    else
        vm controlvm "$VM_NAME" poweroff >/dev/null 2>&1
    fi
}
trap cleanup EXIT

say "dọn kết quả cũ (giữ lại run trước dưới dạng tarball)"
if [ -d "$RESULTS" ]; then
    tar czf "${RESULTS}-prev-$(date +%Y%m%d-%H%M%S).tgz" "$RESULTS" 2>/dev/null || true
fi
rm -rf "$RESULTS"
mkdir -p "$RESULTS" "$SERVE"

BINARY="${BINARY:-$ROOT/target/release/camarelol}"
if [ ! -x "$BINARY" ]; then
    say "binary chưa có — build từ mã nguồn hiện tại"
    cargo build --release -p camarelol || { echo "  FAIL: build thất bại"; exit 1; }
fi
EXP_VERSION="$("$BINARY" --version 2>/dev/null | awk '{print $2}')"
[[ "$EXP_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "  FAIL: không đọc được phiên bản"; exit 1; }
note "binary kiểm tra: $BINARY ($EXP_VERSION)"
cp "$BINARY" "$SERVE/camarelol"
cp "$ROOT/scripts/vm-helper.py" "$SERVE/vm-helper.py"

say "khởi động máy chủ kết quả (cổng $PORT)"
if curl -fsS -m 3 -o /dev/null "http://127.0.0.1:$PORT/camarelol" 2>/dev/null; then
    note "tái dùng listener $PORT đang có sẵn"
else
    python3 "$ROOT/scripts/vm-result-server.py" --port "$PORT" --out "$RESULTS" \
            --serve-dir "$SERVE" > "$RESULTS/server.log" 2>&1 &
    SERVER_PID=$!
    sleep 2
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
        say "máy chủ kết quả không lên:"; cat "$RESULTS/server.log"; exit 1
    fi
    note "server PID $SERVER_PID"
fi

say "tạo VM ($((DISK_SIZE_MB/1024)) GiB — vừa là 'Windows', vừa là đích cài)"
if vm list vms | grep -q "\"$VM_NAME\""; then
    vm controlvm "$VM_NAME" poweroff >/dev/null 2>&1; sleep 2
    vm unregistervm "$VM_NAME" --delete >/dev/null 2>&1
fi
vm createvm --name "$VM_NAME" --ostype ArchLinux_64 --register
vm modifyvm "$VM_NAME" --memory 4096 --cpus 4 --vram 16 \
    --firmware efi \
    --boot1 dvd --boot2 disk --audio-driver none --usb off \
    --clipboard-mode disabled
sleep 3
vm storagectl "$VM_NAME" --name SATA --add sata --controller IntelAhci
vm createmedium disk --filename "${VM_DIR}/${VM_NAME}.vdi" --size "$DISK_SIZE_MB" --format VDI
vm storageattach "$VM_NAME" --storagectl SATA --port 0 --device 0 \
    --type hdd --medium "${VM_DIR}/${VM_NAME}.vdi"
vm storagectl "$VM_NAME" --name IDE --add ide
vm storageattach "$VM_NAME" --storagectl IDE --port 0 --device 0 \
    --type dvddrive --medium "$ISO"

say "chạy nóng NIC/NAT: bật nguồn 40 giây rồi tắt (first boot thiếu mạng)"
vm startvm "$VM_NAME" --type headless >/dev/null 2>&1
sleep 40
vm controlvm "$VM_NAME" poweroff >/dev/null 2>&1
sleep 45

say "khởi động VM (chờ live + IP)"
vm startvm "$VM_NAME" --type headless >/dev/null 2>&1
sleep 70
guest "" 5
guest 'for i in $(seq 1 30); do ip -4 addr show dev enp0s3 | grep -q inet && break; sleep 3; done; ip -4 addr show dev enp0s3 | grep -q inet || systemctl restart systemd-networkd; sleep 2; ip -4 -br addr > /tmp/r-download.txt 2>&1' 120
guest "curl -sS -m 3 -o /dev/null -w prehttp=%{http_code} http://$HOST_ADDR:$PORT/camarelol >> /tmp/r-download.txt 2>&1; echo prestaged >> /tmp/r-download.txt" 10

say "bước 1/7 — tải binary từ máy chủ"
guest "curl -sS -o /root/camarelol http://$HOST_ADDR:$PORT/camarelol >> /tmp/r-download.txt 2>&1 && chmod +x /root/camarelol && echo downloaded >> /tmp/r-download.txt; echo rc=\$? >> /tmp/r-download.txt" 25
post download
check "$RESULTS/step_download.txt" "downloaded" "binary tải về thành công"

say "bước 2/7 — dựng 'Windows' giả trên đĩa (sfdisk + mkfs.fat + marker)"
# Bố trí bằng sector, planner phải khớp đúng:
#   p1 ESP     100 MiB (type U — ESP thật của 'Windows')
#   p2 MSR      16 MiB
#   p3 'C:'      6 GiB giữa đĩa
#   p4 recovery  1 GiB cuối đĩa (type Windows recovery)
#   khe trống   ~17 GiB giữa p3..p4 — nơi camareLOL phải đặt /
# Trên ESP của 'Windows' đặt cả cặp bootmgfw.efi + BCD — efi/20microsoft của
# os-prober chỉ nhận entry khi CẢ HAI có mặt (BCD thật là registry hive của
# Boot Manager; bản giả chỉ cần tồn tại để os-prober chấm entry).
guest "printf 'label: gpt\\nstart=2048, size=204800, type=U\\nstart=206848, size=32768, type=e3c9e316-0b5c-4db8-817d-f92df00215ae\\nstart=307200, size=12582912, type=L\\nstart=48232448, size=2097152, type=de94bba4-06d1-4d40-a16a-bfd50179d6ac\\n' | sfdisk --force /dev/sda > /tmp/r-fakewin.txt 2>&1; SFC=\$?; partprobe /dev/sda >> /tmp/r-fakewin.txt 2>&1; udevadm settle; mkfs.fat -F32 -n SYSTEM /dev/sda1 >> /tmp/r-fakewin.txt 2>&1; mkfs.fat -F32 -n WINDOWS-C /dev/sda3 >> /tmp/r-fakewin.txt 2>&1; mkfs.fat -F32 -n RECOVERY /dev/sda4 >> /tmp/r-fakewin.txt 2>&1; mkdir -p /tmp/esp /tmp/cw /tmp/rec; mount /dev/sda1 /tmp/esp && { mkdir -p /tmp/esp/EFI/Boot; echo original-bootmgr > /tmp/esp/EFI/Boot/.windows-keeper-mark; mkdir -p /tmp/esp/EFI/Microsoft/Boot; echo fake-windows-bootmgr > /tmp/esp/EFI/Microsoft/Boot/bootmgfw.efi; echo fake-bcd > /tmp/esp/EFI/Microsoft/Boot/BCD; }; mount /dev/sda3 /tmp/cw && echo windows-c-marker > /tmp/cw/windows-marker.txt; mount /dev/sda4 /tmp/rec && echo recovery-marker > /tmp/rec/recovery-marker.txt; sync; umount /tmp/esp /tmp/cw /tmp/rec 2>/dev/null; { echo fakewin_rc=\$SFC; lsblk -no NAME,SIZE,PARTTYPENAME /dev/sda; } >> /tmp/r-fakewin.txt" 60
post fakewin
check "$RESULTS/step_fakewin.txt" "fakewin_rc=0" "bảng phân vùng 'Windows' giả viết xong"
check "$RESULTS/step_fakewin.txt" "sda4" "recovery partition hiện trong lsblk"

say "bước 3/7 — quét lại (đĩa giờ là GPT có dữ liệu)"
guest "/root/camarelol --scan > /tmp/survey.json 2>/tmp/scan.err; echo rc=\$? > /tmp/r-scan.txt; grep -c '\"path\"' /tmp/survey.json >> /tmp/r-scan.txt; cat /tmp/scan.err >> /tmp/r-scan.txt" 30
post scan
check "$RESULTS/step_scan.txt" "rc=0" "--scan chạy trên đĩa có phân vùng"

say "bước 4/7 — profile dual-boot (helper chọn đĩa GPT có phân vùng, reuse_esp = true)"
guest "curl -sS -o /root/vm-helper.py http://$HOST_ADDR:$PORT/vm-helper.py >/dev/null 2>&1; CAMARELOL_TEST_DUAL_BOOT=1 python3 /root/vm-helper.py /tmp/survey.json /tmp/profile.toml > /tmp/helper-out.txt 2>&1; HRC=\$?; { cat /tmp/helper-out.txt; grep -E 'reuse_esp|target_disk' /tmp/profile.toml; echo profile_rc=\$HRC; } > /tmp/r-install_profile.txt" 15
post install_profile
check "$RESULTS/step_install_profile.txt" "profile_rc=0" "helper tạo profile dual-boot"
check "$RESULTS/step_install_profile.txt" "reuse_esp = true" "profile bật reuse_esp"

say "bước 4b/7 — preview: 12 pha diễn đủ, không ghi byte nào vào đĩa"
guest "/root/camarelol --scripted /tmp/profile.toml --preview > /tmp/prev.txt 2>&1; echo PREV_RC=\$? > /tmp/r-preview.txt; cat /tmp/prev.txt >> /tmp/r-preview.txt" 45
post preview
check "$RESULTS/step_preview.txt" "PREV_RC=0" "preview dual-boot chạy trọn"
check "$RESULTS/step_preview.txt" "keep existing" "kế hoạch giữ nguyên phân vùng 'Windows' (render plan)"
check "$RESULTS/step_preview.txt" "--new=5:" "danh sách lệnh preview append sgdisk có số tường minh"
if grep -qF -- "--zap-all" "$RESULTS/step_preview.txt"; then
    fail "preview lộ --zap-all — đường ghi erase đang bị dùng cho dual-boot!"
else
    pass "preview không hề thấy --zap-all"
fi

say "bước 5/7 — cài thật kế bên 'Windows' (--scripted --allow-destructive --execute)"
note "cài thật — có thể mất vài phút"
guest "/root/camarelol --scripted /tmp/profile.toml --allow-destructive --execute > /tmp/inst.txt 2>&1; echo INSTALL_RC=\$? > /tmp/r-install.txt; cat /tmp/inst.txt >> /tmp/r-install.txt" 10
post install "${INSTALL_TIMEOUT:-900}"
check "$RESULTS/step_install.txt" "INSTALL_RC=0" "installer thoát 0"
# Report có thể bị in hai lần (TextSink + render cuối); đếm 12 pha DISTINCT.
PHASES=$(grep -oE '^(preflight|partition|format|mount|pacstrap|fstab|bootloader|system-config|accounts|services|initramfs|post-check) +completed' "$RESULTS/step_install.txt" | awk '{print $1}' | sort -u | wc -l)
if [ "$PHASES" = "12" ]; then
    pass "12 pha hoàn tất"
else
    fail "12 pha hoàn tất (chỉ thấy $PHASES pha distinct 'completed')"
fi
check "$RESULTS/step_install.txt" "mode=dual-boot(keep)" "preflight khai rõ mode dual-boot(keep)"
check "$RESULTS/step_install.txt" "appended 1 new partition(s)" "partition phase append, không zap"

say "bước 6/7 — verify: marker + hệ thống vừa cài"
guest "M=/mnt; { echo root_os=\$(head -1 \$M/etc/os-release 2>/dev/null); echo root_src=\$(findmnt -no SOURCE \$M 2>/dev/null); echo esp_efi_dir=\$(test -d \$M/boot/efi/EFI/camareLOL && echo yes || echo no); echo esp_grub=\$(test -s \$M/boot/efi/EFI/camareLOL/grubx64.efi && echo yes || echo no); echo esp_fstab=\$(grep -c '/boot/efi' \$M/etc/fstab); echo os_prober_pkg=\$(pacman -Qr \$M os-prober >/dev/null 2>&1 && echo yes || echo no); echo os_prober_flag=\$(grep -q 'GRUB_DISABLE_OS_PROBER=false' \$M/etc/default/grub && echo yes || echo no); echo grub_cfg=\$(test -s \$M/boot/grub/grub.cfg && echo yes || echo no); echo windows_entry=\$(grep -ci 'windows' \$M/boot/grub/grub.cfg); echo arch_kernel=\$(test -s \$M/boot/vmlinuz-linux && echo yes || echo no); mkdir -p /vm-c /vm-r /vm-e; mount /dev/sda3 /vm-c 2>/dev/null; mount /dev/sda4 /vm-r 2>/dev/null; mount /dev/sda1 /vm-e 2>/dev/null; echo marker_cwin=\$(test -f /vm-c/windows-marker.txt && echo kept || echo GONE); echo marker_rec=\$(test -f /vm-r/recovery-marker.txt && echo kept || echo GONE); echo marker_esp=\$(test -f /vm-e/EFI/Boot/.windows-keeper-mark && echo kept || echo GONE); echo esp_content=\$(head -1 /vm-e/EFI/Boot/.windows-keeper-mark 2>/dev/null); echo bootmgfw=\$(test -s /vm-e/EFI/Microsoft/Boot/bootmgfw.efi && echo kept || echo GONE); echo bcd_kept=\$(test -s /vm-e/EFI/Microsoft/Boot/BCD && echo kept || echo GONE); echo parts=\$(ls /sys/block/sda/ | grep -c '^sda[0-9]'); } > /tmp/r-dualboot.txt 2>&1" 90
post dualboot 300

say "bước 7/7 — kết luận"
R="$RESULTS/step_dualboot.txt"
check "$R" "root_src=/dev/sda5" "root nằm ở phân vùng MỚI (sda5), không đè C:"
for want in "esp_efi_dir=yes" "esp_grub=yes" "os_prober_pkg=yes" "os_prober_flag=yes" \
            "grub_cfg=yes" "arch_kernel=yes" "marker_cwin=kept" "marker_rec=kept" \
            "marker_esp=kept" "esp_content=original-bootmgr" "bootmgfw=kept" \
            "bcd_kept=kept"; do
    if grep -qF "$want" "$R" 2>/dev/null; then pass "$want"; else fail "$want"; fi
done
if [ "$(sed -n 's/^windows_entry=//p' "$R" | tail -1)" -ge 1 ] 2>/dev/null; then
    pass "grub.cfg có entry Windows (os-prober đã chạm đúng bootmgr)"
else
    fail "grub.cfg không có entry Windows: $(sed -n 's/^windows_entry=//p' "$R" | tail -1)"
fi
if [ "$(sed -n 's/^parts=//p' "$R" | tail -1)" = "5" ]; then
    pass "đúng 5 phân vùng (4 của 'Windows' + 1 mới của camareLOL)"
else
    fail "số phân vùng sau cài không phải 5: $(sed -n 's/^parts=//p' "$R" | tail -1)"
fi

say "tổng kết"
if [ "$FAILURES" -ne 0 ]; then
    printf '\n\033[31m=== THẤT BẠI (%s kiểm) ===\033[0m\n' "$FAILURES"
    exit 1
fi
printf '\n\033[32m=== ĐẠT — marker còn nguyên, cài song song với "Windows" sống ===\033[0m\n'
exit 0
