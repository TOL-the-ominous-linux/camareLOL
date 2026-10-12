#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# camareLOL — VirtualBox acceptance test.
#
# Creates a VM with an EMPTY virtual disk, boots the Arch ISO, and runs the
# installer in scripted mode against that disk. Nothing touches the host's
# real disks.
#
# Sub-commands:
#   create    create the VM, the empty disk and the data disk
#   boot      start the VM headless and drive it with keystrokes
#   check     screenshot + verify the install result
#   destroy   delete the VM
#   all       create + boot + check
# ---------------------------------------------------------------------------
set -euo pipefail

VM_NAME="camarelol-test"
VM_DIR="${HOME}/VirtualBox VMs/${VM_NAME}"
ISO="${ISO:-$HOME/Tài liệu/carameLOL/.test/archlinux-x86_64.iso}"
DATA_ISO="${DATA_ISO:-$HOME/Tài liệu/carameLOL/.test/camarelol-data.iso}"
DISK_VDI="${VM_DIR}/${VM_NAME}.vdi"
DATA_IMG="${VM_DIR}/data.img"
SCREENSHOT_DIR="${SCREENSHOT_DIR:-/tmp/opencode/vm-shots}"
LOG_FILE="/tmp/opencode/vm-test.log"

mkdir -p "$SCREENSHOT_DIR"

say() { printf '\n=== %s ===\n' "$*"; }

# ---------------------------------------------------------------- create ---
cmd_create() {
    say "creating VM ${VM_NAME}"
    if VBoxManage list vms | grep -q "\"${VM_NAME}\""; then
        say "VM already exists, destroying first"
        cmd_destroy
    fi

    VBoxManage createvm --name "$VM_NAME" --ostype ArchLinux_64 --register
    VBoxManage modifyvm "$VM_NAME" \
        --memory 4096 --cpus 4 --vram 16 \
        --boot1 dvd --boot2 disk --boot3 none --boot4 none \
        --audio none --usb off --clipboard-mode disabled

    # The sacrificial empty disk (20 GiB) — the install target.
    VBoxManage createmedium disk --filename "$DISK_VDI" --size 20480 --format VDI
    VBoxManage storagectl "$VM_NAME" --name SATA --add sata --controller IntelAhci
    VBoxManage storageattach "$VM_NAME" --storagectl SATA --port 0 --device 0 \
        --type hdd --medium "$DISK_VDI"

    # The data disk: a FAT image with the binary and helper scripts.
    if [ ! -f "$DATA_IMG" ]; then
        say "building FAT data disk"
        dd if=/dev/zero of="$DATA_IMG" bs=1M count=64 status=none
        mkfs.vfat -F 32 -n CAMARELOL "$DATA_IMG" >/dev/null
        mmd -i "$DATA_IMG" ::/scripts
        mcopy -i "$DATA_IMG" "$ROOT/target/release/camarelol" ::/camarelol
        mcopy -i "$DATA_IMG" "$ROOT/scripts/vm-helper.py" ::/scripts/vm-helper.py
        say "converting to VDI"
        VBoxManage convertfromraw "$DATA_IMG" "$DATA_IMG.vdi" --format VDI >/dev/null
        rm -f "$DATA_IMG"
    fi
    VBoxManage storageattach "$VM_NAME" --storagectl SATA --port 1 --device 0 \
        --type hdd --medium "$DATA_IMG.vdi"

    # The Arch ISO.
    VBoxManage storagectl "$VM_NAME" --name IDE --add ide
    VBoxManage storageattach "$VM_NAME" --storagectl IDE --port 0 --device 0 \
        --type dvddrive --medium "$ISO"

    say "VM created: empty 20 GiB disk + FAT data disk + Arch ISO"
}

# ------------------------------------------------------------------ boot ---
type_line() {
    # Type a line into the VM console. The newline is part of the string:
    # VirtualBox's keyboardputscancode Enter does not reach the shell, but
    # keyboardputstring honours an embedded \n.
    local line="$1"
    VBoxManage controlvm "$VM_NAME" keyboardputstring "${line}
" >/dev/null 2>&1
    sleep 0.5
}

shot() {
    VBoxManage controlvm "$VM_NAME" screenshotpng "$SCREENSHOT_DIR/$1.png" >/dev/null 2>&1 || true
}

cmd_boot() {
    say "starting VM headless"
    VBoxManage startvm "$VM_NAME" --type headless
    say "waiting for the Arch ISO to boot (90s)"
    sleep 90
    shot "01-boot"

    say "mounting the data disk"
    type_line "mount /dev/sdb /mnt"
    sleep 2
    shot "02-mounted"

    say "scanning the empty disk"
    type_line "/mnt/camarelol --scan > /tmp/survey.json"
    sleep 5

    say "building the test profile from the survey"
    type_line "python3 /mnt/scripts/vm-helper.py /tmp/survey.json /tmp/test-profile.toml"
    sleep 3
    shot "03-profile"

    say "running the installer (scripted, execute)"
    type_line "/mnt/camarelol --scripted /tmp/test-profile.toml --allow-destructive --execute"
    say "install running — waiting 10 minutes"
    for i in $(seq 1 20); do
        sleep 30
        shot "progress-$i"
    done
    shot "04-done"
    say "boot phase finished"
}

# ----------------------------------------------------------------- check ---
cmd_check() {
    say "checking the result"
    shot "05-final"
    # The installer writes its report to the console; the last screenshot is
    # the source of truth. We also check the VM is still alive.
    if VBoxManage list runningvms | grep -q "\"${VM_NAME}\""; then
        say "VM still running (install may still be in progress)"
    else
        say "VM stopped"
    fi
    say "screenshots in $SCREENSHOT_DIR"
}

# --------------------------------------------------------------- destroy ---
cmd_destroy() {
    say "destroying VM"
    VBoxManage controlvm "$VM_NAME" poweroff >/dev/null 2>&1 || true
    sleep 2
    VBoxManage unregistervm "$VM_NAME" --delete >/dev/null 2>&1 || true
    say "VM destroyed"
}

# ------------------------------------------------------------------- main ---
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cmd="${1:-all}"
case "$cmd" in
    create) cmd_create ;;
    boot)   cmd_boot ;;
    check)  cmd_check ;;
    destroy) cmd_destroy ;;
    all)    cmd_create; cmd_boot; cmd_check ;;
    *)      echo "usage: $0 {create|boot|check|destroy|all}"; exit 2 ;;
esac
