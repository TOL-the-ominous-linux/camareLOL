#!/usr/bin/env bash
# Chạy các bước kiểm tra trên một VM đã bật sẵn, không tạo/khởi động lại.
#
#   ./scripts/vm-run-steps.sh
#
# Tách khỏi vm-acceptance.sh để chạy lại được mà không phải chờ 100 giây boot
# mỗi lần — và để dò từng bước trên một VM đang sống.
set -uo pipefail

VM_NAME="${VM_NAME:-camarelol-acceptance}"
PORT="${PORT:-8899}"
RESULTS="${RESULTS:-/tmp/opencode/vm-accept}"
HOST_ADDR="${HOST_ADDR:-10.0.2.2}"

vm() { VBoxManage "$@" 2>/dev/null; }
say() { printf '\n=== %s ===\n' "$*"; }

guest() {
    vm controlvm "$VM_NAME" keyboardputstring "$1
" >/dev/null
    sleep "${2:-5}"
}
post() {
    guest "curl -sS -m 15 -X POST --data-binary @/tmp/r-$1.txt http://$HOST_ADDR:$PORT/step/$1 >/dev/null 2>&1; echo posted-$1" 5
}

VBoxManage list runningvms | grep -q "\"$VM_NAME\"" || { echo "VM không chạy"; exit 1; }
curl -sS -m 5 -o /dev/null "http://127.0.0.1:$PORT/camarelol" \
    || { echo "máy chủ kết quả không trả lời ở 127.0.0.1:$PORT"; exit 1; }

say "1/5 tải binary"
guest "curl -sS -m 60 -o /root/camarelol http://$HOST_ADDR:$PORT/camarelol > /tmp/r-download.txt 2>&1 && chmod +x /root/camarelol && echo downloaded >> /tmp/r-download.txt; /root/camarelol --version >> /tmp/r-download.txt 2>&1; echo rc=\$? >> /tmp/r-download.txt" 20
post download

say "2/5 --scan trên đĩa thật"
guest "/root/camarelol --scan > /tmp/survey.json 2>/tmp/scan.err; echo rc=\$? > /tmp/r-scan.txt; echo bytes=\$(stat -c%s /tmp/survey.json) >> /tmp/r-scan.txt; cat /tmp/scan.err >> /tmp/r-scan.txt; grep -c path /tmp/survey.json >> /tmp/r-scan.txt; cp /tmp/survey.json $RESULTS/survey.json 2>/dev/null" 20
post scan

say "3/5 --self-test"
guest "/root/camarelol --self-test > /tmp/st.txt 2>&1; echo rc=\$? > /tmp/r-selftest.txt; cat /tmp/st.txt >> /tmp/r-selftest.txt" 25
post selftest

say "4/5 --help không còn cờ cập nhật"
guest "/root/camarelol --help > /tmp/help.txt 2>&1; echo rc=\$? > /tmp/r-help.txt; grep -cE '^    --(check-)?update|rollback-update' /tmp/help.txt >> /tmp/r-help.txt" 12
post help

say "5/5 sinh profile và cài thật"
guest "curl -sS -m 20 -o /root/vm-helper.py http://$HOST_ADDR:$PORT/vm-helper.py >/dev/null 2>&1; python3 /root/vm-helper.py /tmp/survey.json /tmp/profile.toml > /tmp/r-install.txt 2>&1; echo profile_rc=\$? >> /tmp/r-install.txt" 15
post install_profile
say "cài đặt — có thể vài phút"
guest "/root/camarelol --scripted /tmp/profile.toml --allow-destructive --execute > /tmp/inst.txt 2>&1; echo INSTALL_RC=\$? > /tmp/r-install.txt; cat /tmp/inst.txt >> /tmp/r-install.txt" 90
post install

say "kết luận"
python3 "$(dirname "${BASH_SOURCE[0]}")/vm-check-results.py" --results "$RESULTS" --expect-version "${EXPECT_VERSION:-0.1.11}"
