#!/usr/bin/env bash
# camareLOL — real-machine dual-boot auto-install (written for ONE known machine).
#
# WHAT THIS DOES, in plain English:
#   1. verifies it is running as root on an Arch live ISO with internet;
#   2. downloads the 1.0.0 beta binary via the official bootstrap script
#      (beta channel; set CAMARELOL_VERSION to pin another tag);
#   3. inspects /dev/sda and REFUSES unless the layout matches: GPT,
#      every sda partition unmounted, one small vfat EFI System Partition
#      (<= 2 GiB) and one ext4 Linux root (>= 40 GiB);
#   4. if partition 1 still exists (ext4, 40–52 GiB) it asks for a typed
#      confirmation before deleting it to open the gap — and refuses to
#      touch it if it looks like anything else at all;
#   5. builds the keep-everything dual-boot profile from real survey data
#      (reuse_esp = true, bootloader = grub);
#   6. runs --preview first so you can read the plan, then asks for one
#      final typed confirmation before the real install with --execute.
#
# What it NEVER does: touch /dev/nvme*, format the ESP, run
# sgdisk --zap-all, or write anything before the typed confirmations.
#
# Usage (from the live ISO, as root):
#   curl -fsSL https://raw.githubusercontent.com/tamkogay9876-code/camareLOL/refs/heads/main/scripts/dual-boot-auto.sh -o dual-boot-auto.sh
#   bash dual-boot-auto.sh
#
# Overridable via environment: TARGET_DISK (/dev/sda), HOSTNAME (camarelol),
# TIMEZONE (Asia/Ho_Chi_Minh).

set -euo pipefail

TARGET_DISK="${TARGET_DISK:-/dev/sda}"
DISK_BASENAME="$(basename "$TARGET_DISK")"
HOSTNAME_DEF="${HOSTNAME:-camarelol}"
TIMEZONE_DEF="${TIMEZONE:-Asia/Ho_Chi_Minh}"
BOOTSTRAP_URL="https://raw.githubusercontent.com/tamkogay9876-code/camareLOL/refs/heads/main/scripts/camarelol.sh"
MIN_VERSION="1.0.0"

bold() { printf '\033[1m%s\033[0m\n' "$*"; }
warn() { printf '\033[1;33m%s\033[0m\n' "$*"; }
err()  { printf '\033[1;31m%s\033[0m\n' "$*" >&2; }
die()  { err "ABORT: $*"; exit 1; }

# ---------------------------------------------------------------- 1. environment
[ "$(id -u)" -eq 0 ] || die "must run as root (the live ISO logs you in as root automatically)"
[ -e "$TARGET_DISK" ] || die "$TARGET_DISK not found"
bold "== step 1/6: environment OK (root, Arch live) =="

# ---------------------------------------------------------------- 2. internet
bold "== step 2/6: internet check =="
if ! curl -fsI --max-time 20 https://github.com >/dev/null 2>&1; then
    echo "No internet yet. Wi-Fi: run 'iwctl', then 'station wlan0 connect \"<SSID>\"',"
    echo "'exit', and start this script again. Ethernet should just work (DHCP)."
    die "internet unreachable"
fi
echo "internet reachable."

# ---------------------------------------------------------------- 3. fetch the installer
bold "== step 3/6: downloading camareLOL $MIN_VERSION (beta channel) =="
curl -fsSL "$BOOTSTRAP_URL" -o /root/camarelol.sh
# 1.0.0 is the first beta and is published as a GitHub pre-release, so the
# default stable resolver would keep installing 0.1.22. Pin the beta channel;
# CAMARELOL_VERSION can still override it for a later run.
CAMARELOL_CHANNEL="${CAMARELOL_CHANNEL:-beta}" bash /root/camarelol.sh
command -v camarelol >/dev/null 2>&1 || die "camarelol.sh finished but the binary is not on PATH"
VERSION="$(camarelol --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1 || true)"
[ -n "$VERSION" ] || die "could not read the binary version"
if [ "$(printf '%s\n%s\n' "$MIN_VERSION" "$VERSION" | sort -V | head -1)" != "$MIN_VERSION" ]; then
    die "downloaded build $VERSION is too old — the partition/gap display and GRUB auto-switch landed in $MIN_VERSION"
fi
echo "binary ok: $VERSION"

# ---------------------------------------------------------------- 4. survey + safety gates
bold "== step 4/6: surveying $TARGET_DISK =="
camarelol --scan > /tmp/camarelol-survey.json

sgdisk -p "$TARGET_DISK" || die "$TARGET_DISK is not GPT — dual-boot keeps existing partitions, so an MBR disk is out of scope"

MOUNTED=$(findmnt -rno SOURCE,TARGET 2>/dev/null | awk -v d="$TARGET_DISK" '$1 ~ d"[0-9]+$" {print "  " $1 " mounted at " $2}')
[ -z "$MOUNTED" ] || { printf '%s\n' "$MOUNTED"; die "some $TARGET_DISK partitions are mounted — boot the ISO and rerun"; }

# Classify partitions: the ESP (small vfat), the Linux root (big ext4),
# and whatever sits at partition number 1. Bytes, not human sizes, for checks.
ESP_PART=""; ROOT_PART=""; FIRST_PART=""
while read -r name size fstype; do
    num="${name##*[!0-9]}"            # trailing digits: sda2 -> 2, sda10 -> 10
    dev="$TARGET_DISK${name#"$DISK_BASENAME"}"
    case "$fstype" in
        vfat) [ "$size" -le 2147483648 ] && ESP_PART="$dev" ;;
        ext4) [ "$size" -ge 42949672960 ] && ROOT_PART="$dev" ;;
    esac
    [ "$num" = "1" ] && FIRST_PART="$dev"
    true
done < <(lsblk -b -rno NAME,SIZE,FSTYPE "$TARGET_DISK")

[ -n "$ESP_PART" ] && [ -n "$ROOT_PART" ] || {
    echo "expected: one small vfat ESP (<= 2 GiB) + one ext4 Linux root (>= 40 GiB)."
    echo "found:    ESP=[${ESP_PART:-none}]  root=[${ROOT_PART:-none}]"
    die "layout does not match the dual-boot shape this script keeps you safe for"
}
bold "   ESP:   $ESP_PART  — will be REUSED, never formatted"
bold "   root:  $ROOT_PART  — kept exactly as it is"
bold "   free:  the gap beside them is where camareLOL goes"

# ---------------------------------------------------------------- 4b. the one optional deletion
if sgdisk -p "$TARGET_DISK" 2>/dev/null | grep -qE '^ *1 '; then
    first_fs=$(lsblk -no FSTYPE -dn "$FIRST_PART" 2>/dev/null || echo "")
    first_size=$(lsblk -b -o SIZE -dn "$FIRST_PART" 2>/dev/null || echo 0)
    first_label=$(lsblk -no LABEL -dn "$FIRST_PART" 2>/dev/null || echo "")
    if [ "$first_fs" = "ext4" ] && [ "$first_size" -ge 40528947200 ] && [ "$first_size" -le 55834574848 ]; then
        warn "partition 1 still exists: ext4, $(numfmt --to=iec "$first_size" 2>/dev/null || echo "$first_size bytes"), label \"${first_label:-none}\"."
        echo "The owner confirmed this slot empty on 23 Oct 2026; dual-boot needs the gap it leaves."
        printf 'Type ERASE-P1 to delete it (anything else keeps it): '
        read -r ans
        if [ "$ans" = "ERASE-P1" ]; then
            sgdisk --delete=1 "$TARGET_DISK"
            blockdev --rereadpt "$TARGET_DISK"
            bold "partition 1 deleted — the head gap is now visible to the planner."
        else
            warn "kept partition 1. Without the gap the planner will refuse — nothing was written."
            exit 0
        fi
    else
        warn "partition 1 is NOT the known empty ext4 slot (fs=$first_fs, size=$first_size) — touching nothing."
        exit 1
    fi
fi

# ---------------------------------------------------------------- 5. profile + preview
bold "== step 5/6: building the dual-boot profile (keep-everything, GRUB) =="
TARGET="$TARGET_DISK" HOSTNAME="$HOSTNAME_DEF" TIMEZONE="$TIMEZONE_DEF" python3 - <<'PY'
import json, os
from pathlib import Path

TARGET = os.environ["TARGET"]
TIMEZONE = os.environ["TIMEZONE"]
HOSTNAME = os.environ["HOSTNAME"]
survey = json.loads(Path("/tmp/camarelol-survey.json").read_text(encoding="utf-8"))
target = next(d for d in survey if d["path"] == TARGET)
if target.get("partition_table") != "Gpt":
    raise SystemExit("survey says the disk is not GPT — refusing")
identity = target.get("wwn") or target.get("serial") or target["kernel_name"]

profile = f'''# Generated by scripts/dual-boot-auto.sh — real-machine dual-boot profile.
schema_version = 1
language = "en"
system_locale = "en_US.UTF-8"
keymap = "us"
timezone = "{TIMEZONE}"
firmware = "auto"
target_disk = {{ path = "{TARGET}", identity = "{identity}", size_bytes = {target['size_bytes']} }}
filesystem = "ext4"
separate_boot = false
separate_home = false
reuse_esp = true
bootloader = "grub"
desktop = "console"
display_server = "auto"
hostname = "{HOSTNAME}"
root_login_allowed = true
users = []
admin_users = []

[packages]
core = ["base", "linux", "linux-firmware", "base-devel", "sudo", "networkmanager", "vim", "man-db", "man-pages"]
utilities = []
drivers_firmware = []
user_selected = []
aur_packages = []
aur_enabled = false

[services]
audio = "pipewire"
network_manager = true
bluetooth = false
printing = false
ssh = true
zram = true
'''
Path("/tmp/camarelol-dualboot.toml").write_text(profile)
print("profile written: reuse_esp=true, bootloader=grub, target=" + TARGET)
PY
PROFILE=/tmp/camarelol-dualboot.toml

bold "== preview: NOTHING is written — read the plan below =="
camarelol --scripted "$PROFILE" --preview || die "the dual-boot planner REFUSED this plan — nothing was written; read the message above"

printf 'You have read the plan above. Type EXECUTE to install for real: '
read -r ans
[ "$ans" = "EXECUTE" ] || die "aborted before any write — nothing happened"

# ---------------------------------------------------------------- 6. install
bold "== step 6/6: installing (12 phases; a few minutes; be patient) =="
if camarelol --scripted "$PROFILE" --allow-destructive --execute; then
    bold "DONE — 12 phases completed."
    cat <<'EOT'
Your ESP and your existing Linux root were never formatted (the plan said
"keep existing" and that is exactly what ran).

Next:
  1. reboot
  2. GRUB will list the new system and your previous one (os-prober found
     it); the firmware F11 menu also lists both loaders separately.
  3. once booted, check: systemctl status NetworkManager; zramctl;
     and read /etc/NetworkManager/conf.d/00-camarelol.conf exists.
If anything refuses to boot: F11 in firmware brings the previous system
back exactly as it was.
EOT
else
    err "installer exited non-zero — the log above names the failing phase."
    err "Nothing was erased: your ESP and root partitions were only kept or appended to."
    exit 1
fi
