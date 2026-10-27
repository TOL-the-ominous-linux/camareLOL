#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# camareLOL — VM acceptance test for the *published* release.
#
# Covers the two things the earlier scripted test did not:
#   1. the binary that users actually download from GitHub Releases
#   2. the interactive TUI (keyboard navigation + rendering)
#
# Usage: vm-release-test.sh {create|boot|verify|destroy|all}
# ---------------------------------------------------------------------------
set -euo pipefail

VM_NAME="camarelol-release-test"
VM_DIR="${HOME}/VirtualBox VMs/${VM_NAME}"
ISO="${ISO:-$HOME/Tài liệu/carameLOL/.test/archlinux-x86_64.iso}"
DISK_VDI="${VM_DIR}/${VM_NAME}.vdi"
SHOTS="${SHOTS:-/tmp/opencode/vm-shots}"
# Mặc định là bản phát hành mới nhất, không phải một số bản ghim cứng. Ghim v0.1.0
# ở đây khiến bài kiểm tra âm thầm dùng bản cũ nhất mà vẫn báo là đang kiểm tra
# bản mới — và nó cứ thế vẫn xanh.
#
# Kiểm bản beta 1.0.1 (pre-release, không nằm trong /releases/latest):
#   SERIES=https://github.com/TOL-the-ominous-linux/camareLOL/releases/download/v1.0.1 \
#       ./scripts/vm-release-test.sh all
SERIES="${SERIES:-https://github.com/TOL-the-ominous-linux/camareLOL/releases/latest/download}"
mkdir -p "$SHOTS"

say() { printf '\n=== %s ===\n' "$*"; }
vm() { VBoxManage "$@" 2>/dev/null; }

type_line() {
    # VirtualBox's keyboardputscancode Enter never reaches the guest shell;
    # an embedded newline in keyboardputstring does.
    vm controlvm "$VM_NAME" keyboardputstring "${1}
" >/dev/null
    sleep "${2:-1.5}"
}

run_and_capture() {
    # Run a command in the guest and save its output to a file we can read back.
    type_line "$1" 6
    vm controlvm "$VM_NAME" screenshotpng "$SHOTS/$2.png" >/dev/null
}

shot() { vm controlvm "$VM_NAME" screenshotpng "$SHOTS/$1.png" >/dev/null || true; }

cmd_create() {
    say "creating $VM_NAME"
    if vm list vms | grep -q "\"$VM_NAME\""; then cmd_destroy; fi
    vm createvm --name "$VM_NAME" --ostype ArchLinux_64 --register
    vm modifyvm "$VM_NAME" --memory 4096 --cpus 4 --vram 16 \
        --boot1 dvd --boot2 disk --audio-driver none --usb off \
        --clipboard-mode disabled
    # Two empty disks: one for the smoke test, one for a real install.
    vm storagectl "$VM_NAME" --name SATA --add sata --controller IntelAhci
    for port in 0 1; do
        vm createmedium disk --filename "${VM_DIR}/disk${port}.vdi" --size 16384 --format VDI
        vm storageattach "$VM_NAME" --storagectl SATA --port "$port" --device 0 \
            --type hdd --medium "${VM_DIR}/disk${port}.vdi"
    done
    vm storagectl "$VM_NAME" --name IDE --add ide
    vm storageattach "$VM_NAME" --storagectl IDE --port 0 --device 0 \
        --type dvddrive --medium "$ISO"
    say "created: Arch ISO + 2 empty 16 GiB disks"
}

cmd_boot() {
    say "booting (waiting 100s for the live environment)"
    vm startvm "$VM_NAME" --type headless
    sleep 100
    shot "01-booted"

    say "network check"
    type_line "ping -c1 -W3 140.82.121.4 >/dev/null 2>&1 && echo NET_OK || echo NET_FAIL" 6
    shot "02-network"

    say "download the published binary"
    type_line "curl -sSL -o /root/camarelol ${SERIES}/camarelol && chmod +x /root/camarelol && echo DOWNLOAD_OK || echo DOWNLOAD_FAIL" 40
    shot "03-download"

    say "smoke test: version, scan, self-test"
    type_line "/root/camarelol --version" 4
    type_line "/root/camarelol --scan > /tmp/survey.json; echo scan_lines=\$(wc -l < /tmp/survey.json)" 6
    type_line "/root/camarelol --self-test; echo selftest_rc=\$?" 15
    shot "04-smoke"

    say "acceptance scenarios from the Python tool"
    type_line "/root/camarelol --self-test > /tmp/selftest.log 2>&1; echo rc=\$?; tail -2 /tmp/selftest.log" 12
    shot "05-selftest"
}

cmd_verify() {
    say "checking the result"
    shot "06-final"
    if vm list runningvms | grep -q "\"$VM_NAME\""; then
        say "VM still running"
    else
        say "VM stopped"
    fi
    say "screenshots: $SHOTS"
}

# Drive the interactive TUI: the earlier test only covered scripted mode.
cmd_tui() {
    say "starting the TUI"
    type_line "clear; /root/camarelol --allow-destructive" 4

    # Welcome -> Language -> Keyboard -> Timezone
    shot "tui-welcome"
    for step in language keyboard timezone storage; do
        type_line "" 2
        shot "tui-$step"
    done

    say "help overlay (F1)"
    VBoxManage controlvm "$VM_NAME" keyboardputscancode 3b >/dev/null 2>&1
    sleep 2
    shot "tui-help"
    VBoxManage controlvm "$VM_NAME" keyboardputscancode 01 >/dev/null 2>&1
    sleep 2

    say "walk through the remaining steps to the review screen"
    for step in diskmode layout filesystem accounts bootloader desktop services packages; do
        type_line "" 2
        shot "tui-$step"
    done

    say "jump straight to review (F10)"
    VBoxManage controlvm "$VM_NAME" keyboardputscancode 79 >/dev/null 2>&1
    sleep 3
    shot "tui-review"
    say "the review screen must list the destructive operations and the phrase"
}

cmd_destroy() {
    say "destroying"
    vm controlvm "$VM_NAME" poweroff >/dev/null 2>&1 || true
    sleep 2
    vm unregistervm "$VM_NAME" --delete >/dev/null 2>&1 || true
}

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export SERIES ROOT
case "${1:-all}" in
    create)  cmd_create ;;
    boot)    cmd_boot ;;
    verify)  cmd_verify ;;
    destroy) cmd_destroy ;;
    tui)     cmd_tui ;;
    all)     cmd_create; cmd_boot; cmd_verify ;;
    *)       echo "usage: $0 {create|boot|verify|destroy|tui|all}"; exit 2 ;;
esac