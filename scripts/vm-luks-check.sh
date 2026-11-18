#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# camareLOL — kiểm LUKS2 + btrfs trên đĩa ĐÃ CÀI, chạy trong máy khách.
#
#   bash vm-luks-check.sh http://10.0.2.2:8899 > /tmp/r-luks.txt 2>&1
#
# Được scripts/vm-acceptance.sh gọi khi LUKS=1, sau khi cài xong và trước khi
# khởi động lại.
#
# Vì sao phải mở container bằng chính mật khẩu đã dùng để tạo nó: `luksDump`
# chỉ nói container tồn tại. Nó không nói mật khẩu mở được, không nói bên trong
# là btrfs, và không nói `@`/`@home`/`@snapshots` có thật. Ba thứ sau là thứ
# người dùng sống với — nên chúng được kiểm bằng cách mở ra và nhìn vào.
#
# Mỗi dòng in ra là một `key=value`; scripts/vm-check-results.py đọc đúng những
# khoá đó và báo hỏng nếu thiếu khoá nào, nên một bước chết giữa chừng không
# thể trông giống một bước đạt.
# ---------------------------------------------------------------------------
set -uo pipefail

BASE_URL="${1:-http://10.0.2.2:8899}"
PROFILE=/tmp/profile.toml
PASS=/tmp/luks.pass
MNT=/vmroot
BOOTMNT=/vmboot

# Đĩa đích lấy từ chính profile, không đoán: cùng nguồn với bước zram.
DISK=$(grep -oE '/dev/(sd|vd)[a-z]+' "$PROFILE" 2>/dev/null | head -n1)
echo "disk=${DISK:-none}"

# Container nằm trên phân vùng gốc; tìm theo dấu hiệu LUKS thay vì theo số thứ
# tự phân vùng, vì thứ tự đó phụ thuộc firmware và separate_boot.
LUKS_PART=$(blkid -t TYPE=crypto_LUKS -o device 2>/dev/null | head -n1)
echo "luks_part=${LUKS_PART:-none}"
if [ -n "$LUKS_PART" ]; then echo "luks_container=yes"; else echo "luks_container=no"; fi

if [ -n "$LUKS_PART" ] &&
   cryptsetup luksDump "$LUKS_PART" 2>/dev/null | grep -qE '^Version:[[:space:]]+2$'; then
    echo "luks_v2=yes"
else
    echo "luks_v2=no"
fi

# Bộ cài mở container tên `cryptroot` và để nguyên nó cho tới lần boot sau. Một
# lần cài hỏng có thể để nó mở dở, và mở lại /dev/sda3 khi đã map chỉ trả
# "already in use" — trông y như sai mật khẩu. Đóng trước để phép thử là phép
# thử thật: mật khẩu này có mở được container không.
install_mapper_closed=no
if cryptsetup status cryptroot >/dev/null 2>&1; then
    umount -R /mnt 2>/dev/null
    cryptsetup close cryptroot 2>/dev/null && install_mapper_closed=yes
fi
echo "install_mapper_closed=$install_mapper_closed"

# Mở bằng đúng mật khẩu mà bộ cài đã dùng. Sai mật khẩu ở đây nghĩa là cái được
# cài không mở được bằng thứ người dùng gõ — hỏng theo cách tệ nhất.
if [ -n "$LUKS_PART" ] &&
   cryptsetup open --key-file "$PASS" "$LUKS_PART" cryptcheck 2>/tmp/luks-open.err; then
    echo "luks_unlock=yes"
else
    echo "luks_unlock=no"
    sed -n '1,3p' /tmp/luks-open.err 2>/dev/null
fi

mkdir -p "$MNT"
umount "$MNT" 2>/dev/null
if mount -o subvolid=5 /dev/mapper/cryptcheck "$MNT" 2>/tmp/luks-mount.err; then
    echo "btrfs_top=yes"
else
    echo "btrfs_top=no"
    sed -n '1,3p' /tmp/luks-mount.err 2>/dev/null
fi

if [ "$(blkid -s TYPE -o value /dev/mapper/cryptcheck 2>/dev/null)" = "btrfs" ]; then
    echo "btrfs_fs=yes"
else
    echo "btrfs_fs=no"
fi

# Tên subvolume là hợp đồng với /etc/fstab và với snapshot: sai tên thì máy vẫn
# boot nhưng /home hoặc /.snapshots nằm trong subvolume của /.
for pair in "@:root" "@home:home" "@snapshots:snapshots"; do
    sub=${pair%%:*}
    key=${pair##*:}
    if btrfs subvolume list "$MNT" 2>/dev/null | grep -qE " path ${sub}\$"; then
        echo "subvol_${key}=yes"
    else
        echo "subvol_${key}=no"
    fi
done

TARGET="$MNT/@"
if [ -d "$TARGET/etc" ]; then
    # genfstab ghi `subvol=/@`, không phải `subvol=@`. Cả hai đều đúng với btrfs,
    # nên phép kiểm nhận cả hai thay vì đỏ vì cách viết.
    grep -qE 'subvol=/?@([,[:space:]]|$)' "$TARGET/etc/fstab" \
        && echo "fstab_subvol=yes" || echo "fstab_subvol=no"
    grep -qE '^cryptroot[[:space:]]+UUID=[0-9a-fA-F-]{36}' "$TARGET/etc/crypttab" \
        && echo "crypttab=yes" || echo "crypttab=no"
    # UUID trong crypttab phải là UUID của chính container này: một crypttab
    # trỏ nhầm ổ cũng "trông đúng" và chết ở lần boot đầu.
    part_uuid=$(cryptsetup luksUUID "$LUKS_PART" 2>/dev/null || echo "")
    if [ -n "$part_uuid" ] && grep -q "$part_uuid" "$TARGET/etc/crypttab" 2>/dev/null; then
        echo "crypttab_uuid_match=yes"
    else
        echo "crypttab_uuid_match=no"
    fi
    # Chỉ dòng HOOKS= đang hoạt động mới tính. mkinitcpio.conf mặc định liệt kê
    # mọi hook trong comment, nên grep cả tệp sẵn sàng "thấy" `encrypt` ở một
    # hệ không hề cấu hình nó — đúng cái bẫy đã làm hỏng lần chạy 2026-11-16.
    if grep -qE '^HOOKS=.*(^|[ (])(sd-encrypt|encrypt)([ )]|$)' "$TARGET/etc/mkinitcpio.conf" \
       && grep -qE '^HOOKS=.*keyboard' "$TARGET/etc/mkinitcpio.conf"; then
        echo "hook_encrypt=yes"
    else
        echo "hook_encrypt=no"
    fi
    # Chấp nhận cả hai cách viết: `cryptdevice=` của hook busybox và
    # `rd.luks.name=` của sd-encrypt. Chỉ một trong hai là đủ để initramfs mở
    # container — nhưng phải là UUID của CHÍNH ổ này, nên UUID được kiểm riêng.
    if grep -qE '^GRUB_CMDLINE_LINUX=.*(cryptdevice=UUID=|rd\.luks\.name=)' \
            "$TARGET/etc/default/grub" 2>/dev/null &&
       grep -q "$part_uuid" "$TARGET/etc/default/grub" 2>/dev/null; then
        echo "grub_cmdline=yes"
    else
        echo "grub_cmdline=no"
    fi
    # curl là thứ unit báo cáo dùng để gọi về host; thiếu nó thì bằng chứng
    # boot không bao giờ tới, và bài kiểm tra phải nói ra điều đó ở đây.
    if pacman -Qr "$TARGET" curl >/dev/null 2>&1; then
        echo "curl_pkg=yes"
    else
        echo "curl_pkg=no"
    fi
else
    echo "target_tree=no"
fi

# Kernel + initramfs phải nằm NGOÀI container: GRUB không đọc được thứ bên
# trong LUKS, nên đây là cái làm cho lần boot đầu tiên có thể xảy ra.
#
# Không đoán phân vùng boot theo loại filesystem: /boot riêng được tạo với
# CHÍNH filesystem của root (nhánh này là btrfs), nên "phân vùng ext4" vừa sai
# vừa là một cách nói sai về layout. Dấu hiệu thật là kernel nằm trên đó.
BOOT_PART=""
mkdir -p "$BOOTMNT"
for part in $(lsblk -lnpo NAME,TYPE "$DISK" 2>/dev/null | awk '$2 == "part" { print $1 }'); do
    [ "$part" = "$LUKS_PART" ] && continue
    if mount -o ro "$part" "$BOOTMNT" 2>/dev/null; then
        if [ -e "$BOOTMNT/vmlinuz-linux" ]; then
            BOOT_PART=$part
            echo "boot_kernel=yes"
            if ls "$BOOTMNT"/initramfs-linux*.img >/dev/null 2>&1; then
                echo "boot_initramfs=yes"
            else
                echo "boot_initramfs=no"
            fi
            umount "$BOOTMNT" 2>/dev/null
            break
        fi
        umount "$BOOTMNT" 2>/dev/null
    fi
done
echo "boot_part=${BOOT_PART:-none}"
if [ -z "$BOOT_PART" ]; then
    echo "boot_kernel=no"
    echo "boot_initramfs=no"
fi

# Unit báo về host: không có nó thì lần khởi động lại không thể sinh bằng chứng.
if [ -d "$TARGET/etc/systemd/system" ]; then
    if curl -sS -o "$TARGET/etc/systemd/system/camarelol-boot-report.service" \
            "$BASE_URL/camarelol-boot-report.service" 2>/dev/null; then
        echo "boot_unit_file=yes"
    else
        echo "boot_unit_file=no"
    fi
    systemctl --root="$TARGET" enable camarelol-boot-report.service >/dev/null 2>&1
    if [ "$(systemctl --root="$TARGET" is-enabled camarelol-boot-report.service 2>/dev/null)" = "enabled" ]; then
        echo "boot_unit_enabled=yes"
    else
        echo "boot_unit_enabled=no"
    fi
else
    echo "boot_unit_file=no"
    echo "boot_unit_enabled=no"
fi

# Để các bước sau (zram, desktop) dùng lại: /vmroot trỏ vào hệ ĐÃ CÀI, không
# phải subvolid=5. Không đóng mapper `cryptcheck` — nó biến mất cùng lần reboot,
# và đóng nó ở đây chỉ khiến bước sau phải mở lại bằng mật khẩu lần nữa.
umount "$MNT" 2>/dev/null
if mount -o subvol=/@ /dev/mapper/cryptcheck "$MNT" 2>/dev/null; then
    echo "root_mounted_for_next_steps=yes"
else
    echo "root_mounted_for_next_steps=no"
fi
