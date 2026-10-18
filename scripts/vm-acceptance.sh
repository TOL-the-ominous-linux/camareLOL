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

VM_NAME="${VM_NAME:-camarelol-acceptance}"
VM_DIR="${HOME}/VirtualBox VMs/${VM_NAME}"

# Desktop run: DESKTOP=hyprland DISPLAY_SERVER=wayland DISPLAY_MANAGER=greetd
# PRINTING=true. Rỗng (mặc định) = console-only, đúng bài CI cũ.
DESKTOP="${DESKTOP:-}"
DISPLAY_SERVER="${DISPLAY_SERVER:-}"
DISPLAY_MANAGER="${DISPLAY_MANAGER:-}"
PRINTING="${PRINTING:-false}"

# greetd cần biết compositor để viết config; suy ra từ profile desktop.
SESSION_CMD=""; DM_PKG="greetd-tuigreet"
case "$DESKTOP" in
    hyprland) SESSION_CMD="Hyprland" ;;
    sway)     SESSION_CMD="sway" ;;
esac
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

post() { # post <tên> [timeout_s] — chờ TỆP về thật sự thay vì ngủ cứng
    # Gõ lệnh post rồi CHỜ $RESULTS/step_<tên>.txt xuất hiện (máy chủ tạo
    # tệp tại chỗ khi nhận). Sleep cứng 4 giây từng đối với thời gian thật
    # của khách: cài thật dài hơn 60 giây → checker đọc trước khi tệp về
    # (run 6), và post cuối gõ vào hàng đợi rồi VM bị tắt mất — server
    # không bao giờ nhận step_zram. Chờ tệp = đồng bộ theo sự thật; hết giờ
    # thì checker tự khai "không có kết quả". Curl in mã + stderr ra màn
    # hình nên tầng nào hỏng cũng có tên.
    guest "X=\$(curl -sS -m 8 -o /dev/null -w '%{http_code}' -X POST --data-binary @/tmp/r-$1.txt http://$HOST_ADDR:$PORT/step/$1 2>&1); echo post-$1-rc=\$?; echo post-$1 http=\$X; echo done-$1" 1
    local to="${2:-45}" waited=0
    while [ $waited -lt $to ]; do
        [ -e "$RESULTS/step_$1.txt" ] && return 0
        sleep 2
        waited=$((waited + 2))
    done
    # Hết giờ mà không thấy tệp: nói ra. Im lặng khiến checker đọc tệp rỗng rồi
    # kết luận đỏ, trong khi tệp về vài giây sau đó (đo 2026-10-17, lần build
    # 0.1.15: step_zram.txt về lúc 15:13:49, sau khi kết luận đã in).
    say "  tệp $RESULTS/step_$1.txt chưa về sau ${to}s — checker sẽ báo hỏng, kể cả khi tệp tới sau đó"
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
BINARY="$ROOT/out/release/camarelol"
[ -x "$BINARY" ] || { echo "  FAIL: binary không có hoặc không chạy được: $BINARY"; exit 1; }
# Phiên bản kỳ vọng lấy từ chính binary sẽ kiểm — không sao chép số ra chỗ
# khác: 0.1.11 từng cài mimes lệch để script ghim cứng --expect-version 0.1.11,
# và chạy sai bài. Cờ --version của binary in 'camareLOL <v> (tag …)'.
EXP_VERSION="$("$BINARY" --version 2>/dev/null | awk '{print $2}')"
[[ "$EXP_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "  FAIL: không đọc được phiên bản từ binary: '$EXP_VERSION'"; exit 1; }
note "binary kiểm tra: $BINARY (phiên bản $EXP_VERSION)"
# Chép đúng những gì khách sẽ dùng. Không dùng bản trong kho: bài kiểm tra này
# kiểm tra *thứ đã phát hành*, và chỉ máy chủ mới biết thứ đó là gì.
cp "$ROOT/out/release/camarelol" "$SERVE/camarelol"
cp "$ROOT/scripts/vm-helper.py" "$SERVE/vm-helper.py"

say "khởi động máy chủ kết quả (cổng $PORT)"
# Server sinh từ shell nền (opencode background) bị KHÁCH không nhìn thấy:
# host 127.0.0.1 vẫn 200 nhưng guest qua NAT luôn 000 — nguyên nhân hai run
# chết trắng đầu tiên (mọi probe thủ công khởi từ foreground đều 200). Vì
# vậy ưu tiên listener khỏe khởi từ foreground: nếu cổng đã có listener
# trả 200 cho /camarelol thì tái dùng và không tự sinh server.
if curl -fsS -m 3 -o /dev/null "http://127.0.0.1:$PORT/vm-helper.py" 2>/dev/null; then
    note "  tái dùng listener $PORT đang có sẵn (khởi từ foreground)"
else
    python3 "$ROOT/scripts/vm-result-server.py" --port "$PORT" --out "$RESULTS" \
            --serve-dir "$SERVE" > "$RESULTS/server.log" 2>&1 &
    SERVER_PID=$!
    sleep 2
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
        say "máy chủ kết quả không lên:"; cat "$RESULTS/server.log"; exit 1
    fi
    note "  server tự sinh PID $SERVER_PID. Chạy harness từ shell nền mà run chết trắng thì khởi listener từ foreground: nohup python3 scripts/vm-result-server.py --port $PORT --out $RESULTS --serve-dir $SERVE &"
fi
say "tạo VM"
if vm list vms | grep -q "\"$VM_NAME\""; then
    vm controlvm "$VM_NAME" poweroff >/dev/null 2>&1; sleep 2
    vm unregistervm "$VM_NAME" --delete >/dev/null 2>&1
fi
vm createvm --name "$VM_NAME" --ostype ArchLinux_64 --register
vm modifyvm "$VM_NAME" --memory 4096 --cpus 4 --vram 16 \
    --boot1 dvd --boot2 disk --audio-driver none --usb off \
    --clipboard-mode disabled
# VM mới tạo rồi startvm ngay lập tức đã gặp lỗi NIC/DHCP lần đầu tạm thời —
# nghỉ một hơi giữa các lệnh cấu hình VM với startvm.
sleep 3
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

# Khối từng sửa .vbox để thêm `localhost-reachable="true"` bị bỏ — bằng chứng
# trên VBox 7.2.20: thuộc tính KHÔNG có trong schema, load nếu có cũng bị lược
# và power-off đầu tiên xoá sạch (căn cứ: .vbox sau lần chạy 0.1.13 đầu là
# `<NAT/>` trơn, dù thông báo đã in "bật"). Còn thanh toán cho việc nó không
# cần tới: probe trên chính VM này với .vbox trơn — `GET=200`, 10.0.2.2 tới
# được loopback máy chủ mặc định. Không giữ khối nào giả thành công ở đây.

# Lỗi thật của hai lần chạy trắng đầu tiên (0.1.13, hai lần liên tiếp): VM
# *vừa create* bấm nguồn lên mà NIC/NAT chưa tự dựng xong — mỗi curl từ khách
# đều 000, không một kết quả nào về host, trong khi mọi lần boot sau khi đã
# tắt–bật lại đều có mạng chuẩn (probe: GET=200). Vậy chạy nóng một vòng trước
# đích thật: đây là công việc, không phải may rủi.
say "chạy nóng NIC/NAT: bật nguồn 40 giây rồi tắt (first boot của VM vừa tạo không có mạng)"
vm startvm "$VM_NAME" --type headless >/dev/null 2>&1
sleep 40
vm controlvm "$VM_NAME" poweroff >/dev/null 2>&1
# Nghỉ 45 giây sau poweroff: run 3 và run 4 khi nghỉ 3 giây vẫn chết trắng cả
# session (mọi curl 000, server log trống), trong khi chính VM đó được boot
# lại sau ~10 phút nghỉ thì có 10.0.2.15 + GET 200 ngay (probe 2026-10-17,
# ảnh /tmp/vm-su/probeMark.png). Chuẩn của sự chênh: khoảng nghỉ dài hơn để
# NAT/DHCP VirtualBox dựng lại phiên; kể cả bằng VM instance xác định như nhau.
sleep 45

say "khởi động VM (chờ môi trường live, rồi chờ IP thật sự)"
vm startvm "$VM_NAME" --type headless
sleep 70   # hệ live chuyển ra tty1, networkd chạy DHCP ban đầu
guest "" 5   # Enter để đảm bảo có prompt
# Đừng tin một giấc ngủ 100 giây: lần chạy đầu của phiên này, VM dậy mà không
# có IP nào, mọi curl đều thất bại và cả bảy bước về tay trắng — trong khi
# không một dấu hiệu nào hiện lên. Chờ cho tới khi enp0s3 có IP thật; sau 90
# giây vẫn trống thì khởi lại networkd một lần và chờ tiếp. Ảnh chụp IP của
# khoảnh khắc này đi theo tệp r-download.txt — vậy nếu tải thất bại thì tệp
# thất bại tự khai báo mạng đã ở tình trạng nào.
guest 'for i in $(seq 1 30); do ip -4 addr show dev enp0s3 | grep -q inet && break; sleep 3; done; ip -4 addr show dev enp0s3 | grep -q inet || { systemctl restart systemd-networkd; sleep 12; }; ip -4 -br addr 2>&1 | tee /tmp/r-download.txt' 120
# Probe guest→server in THẲNG lên màn hình: run chết trắng cũng tự khai báo
# tầng nào hỏng — 000 nghĩa là kết nối không tới server, 200 là thấy được.
guest "curl -sS -m 3 -o /dev/null -w prehttp=%{http_code} http://$HOST_ADDR:$PORT/camarelol; echo pre-done" 10
# Ảnh chụp NGAY khoảnh khắc preflight: nếu run chết trắng sau đó, 05-preflight.png
# vẫn còn nguyên trạng thái IP/giải mã mà 99-final.png cuối run đã lướt qua.
vm controlvm "$VM_NAME" screenshotpng "$RESULTS/05-preflight.png" >/dev/null 2>&1 || true

# ------------------------------------------------------------- các bước

say "bước 1/6 — tải binary từ máy chủ"
guest "curl -sS -o /root/camarelol http://$HOST_ADDR:$PORT/camarelol >> /tmp/r-download.txt 2>&1 && chmod +x /root/camarelol && echo downloaded >> /tmp/r-download.txt; /root/camarelol --version >> /tmp/r-download.txt 2>&1; echo rc=\$? >> /tmp/r-download.txt" 25
post download

say "bước 2/6 — quét thiết bị thật (--scan)"
guest "/root/camarelol --scan > /tmp/survey.json 2>/tmp/scan.err; echo rc=\$? > /tmp/r-scan.txt; echo bytes=\$(stat -c%s /tmp/survey.json) >> /tmp/r-scan.txt; cat /tmp/scan.err >> /tmp/r-scan.txt; grep -c path /tmp/survey.json >> /tmp/r-scan.txt" 20
post scan

say "bước 3/6 — self-test"
guest "/root/camarelol --self-test > /tmp/st.txt 2>&1; echo rc=\$? > /tmp/r-selftest.txt; cat /tmp/st.txt >> /tmp/r-selftest.txt" 25
post selftest

say "bước 4/6 — --help không còn cờ cập nhật"
guest "/root/camarelol --help > /tmp/help.txt 2>&1; echo rc=\$? > /tmp/r-help.txt; grep -cE '^    --(check-)?update|rollback-update' /tmp/help.txt >> /tmp/r-help.txt; grep -A3 'UPDATING' /tmp/help.txt >> /tmp/r-help.txt" 12
post help

say "bước 5/6 — cài thật vào đĩa trống (--scripted --execute)"
guest "curl -sS -o /root/vm-helper.py http://$HOST_ADDR:$PORT/vm-helper.py >/dev/null 2>&1; echo helper_dlc=\$? bytes=\$(stat -c%s /root/vm-helper.py 2>/dev/null); CAMARELOL_TEST_DESKTOP=$DESKTOP CAMARELOL_TEST_DISPLAY_SERVER=$DISPLAY_SERVER CAMARELOL_TEST_DISPLAY_MANAGER=$DISPLAY_MANAGER CAMARELOL_TEST_PRINTING=$PRINTING python3 /root/vm-helper.py /tmp/survey.json /tmp/profile.toml > /tmp/helper-out.txt 2>&1; HRC=\$?; echo helper_rc=\$HRC; cat /tmp/helper-out.txt; grep -E '^(desktop|display_server|display_manager) =' /tmp/profile.toml >> /tmp/helper-out.txt; { cat /tmp/helper-out.txt; echo profile_rc=\$HRC; } > /tmp/r-install_profile.txt" 15
post install_profile
note "chạy cài đặt — có thể mất vài phút"
guest "/root/camarelol --scripted /tmp/profile.toml --allow-destructive --execute > /tmp/inst.txt 2>&1; echo INSTALL_RC=\$? > /tmp/r-install.txt; cat /tmp/inst.txt >> /tmp/r-install.txt" 10
post install "${INSTALL_TIMEOUT:-600}"

say "bước 6/6 — kiểm trên hệ thống ĐÃ CÀI: unit, gói, config của zram và ssh"
# Hai bản 0.1.11/0.1.12 cài "xong" mà thiếu thật. Những gì quyết định tính năng
# sống là trên đĩa đích, không phải mã thoát của trình cài: unit file có thật,
# symlink enable có thật, binary/generator có thật, config có thật. Cá nhân
# /dev/sdX lấy từ profile để không đoán đĩa.
guest "D=\$(grep -om1 '/dev/sd[a-z]' /tmp/profile.toml); [[ \$D ]] || D=/dev/sda; ROOTP=\$(findmnt -no SOURCE /mnt 2>/dev/null); [[ -n \"\$ROOTP\" ]] || ROOTP=\${D}2; M=/vmroot; mkdir -p \$M 2>/dev/null; mount \"\$ROOTP\" \$M 2>/dev/null; { test -e \$M/usr/lib/systemd/system/systemd-zram-setup@.service && echo zram_unit=yes || echo zram_unit=no; [[ \$(systemctl --root=\$M is-enabled systemd-zram-setup@zram0 2>/dev/null) == enabled ]] && echo zram_wants=yes || echo zram_wants=no; test -e \$M/usr/lib/systemd/system-generators/zram-generator && echo zram_bin=yes || echo zram_bin=no; test -e \$M/etc/systemd/zram-generator.conf && echo zram_conf=yes || echo zram_conf=no; test -e \$M/usr/lib/systemd/system/sshd.service && echo sshd_unit=yes || echo sshd_unit=no; [[ \$(systemctl --root=\$M is-enabled sshd 2>/dev/null) == enabled ]] && echo sshd_wants=yes || echo sshd_wants=no; pacman -Qr \$M openssh >/dev/null 2>&1 && echo openssh=yes || echo openssh=no; } > /tmp/r-zram.txt 2>&1" 25
post zram 300

# Phần display chỉ có nghĩa khi profile không phải console. Bỏ trống thì bài
# kiểm tra giữ nguyên phạm vi cũ — thêm một bước luôn "đạt" vì không làm gì
# chính là cách làm bài kiểm tra thành nghĩa trang.
if [ -n "$DESKTOP" ]; then
    say "bước 7/7 — display stack trên hệ ĐÃ CÀI (profile=$DESKTOP)"
    # `systemctl --root is-enabled` là câu trả lời của chính systemd, không đoán
    # vị trí symlink. Bài kiểm tra trước nhìn `multi-user.target.wants/` rồi báo
    # greetd "no" — sai: greetd dùng `Alias=display-manager.service`, không có
    # symlink nào trong .wants, và nó vẫn chạy. Unit không cài trả "not-found"
    # nên đây phân biệt được thật với giả.
    guest "D=\$(grep -om1 '/dev/sd[a-z]' /tmp/profile.toml); [[ \$D ]] || D=/dev/sda; ROOTP=\$(findmnt -no SOURCE /mnt 2>/dev/null); [[ -n \"\$ROOTP\" ]] || ROOTP=\${D}2; M=/vmroot; mkdir -p \$M 2>/dev/null; mount \"\$ROOTP\" \$M 2>/dev/null; { for u in $DISPLAY_MANAGER cups; do printf 'en_%s=' \"\$u\"; systemctl --root=\$M is-enabled \$u 2>/dev/null | tr -d '\n'; echo; done; test -e \$M/usr/lib/systemd/system/$DISPLAY_MANAGER.service && echo dm_unit=yes || echo dm_unit=no; test -L \$M/etc/systemd/system/display-manager.service && echo dm_alias=yes || echo dm_alias=no; test -e \$M/etc/greetd/config.toml && echo greetd_conf=yes || echo greetd_conf=no; grep -q '$SESSION_CMD' \$M/etc/greetd/config.toml 2>/dev/null && echo greetd_session=yes || echo greetd_session=no; pacman -Qr \$M $DESKTOP >/dev/null 2>&1 && echo desktop_pkg=yes || echo desktop_pkg=no; pacman -Qr \$M $DM_PKG >/dev/null 2>&1 && echo dm_pkg=yes || echo dm_pkg=no; pacman -Qr \$M wayland >/dev/null 2>&1 && echo wayland=yes || echo wayland=no; pacman -Qr \$M xorg-xwayland >/dev/null 2>&1 && echo xwayland=yes || echo xwayland=no; pacman -Qr \$M cups >/dev/null 2>&1 && echo cups=yes || echo cups=no; } > /tmp/r-desktop.txt 2>&1" 25
    post desktop 300
fi

say "đọc kết quả và kết luận"
python3 "$ROOT/scripts/vm-check-results.py" --results "$RESULTS" --expect-version "$EXP_VERSION" \
    $([ -n "$DESKTOP" ] && echo --expect-desktop "$DESKTOP")
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
