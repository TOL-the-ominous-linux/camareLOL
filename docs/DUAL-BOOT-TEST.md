# Hướng dẫn test dual-boot Arch không đụng Storage

> **Cập nhật 22 Oct 2026 — dual-boot đã chạy thật trong VM.**
> `scripts/vm-dualboot.sh` cài camareLOL cạnh một "Windows" giả bên trong VM
> `camarelol-dualboot`: bảng phân vùng ESP/MSR/C:/recovery dựng bằng sfdisk,
> ba tệp đánh dấu (bootmgr trên ESP, file data trên C:, marker trên recovery)
> **còn nguyên sau khi cài** (run 4, log `/tmp/opencode/vm-dualboot-run4.log`).
> root mới ở `sda5`, GRUB đặt lên ESP cũ (không format lại), fstab mount
> `/boot/efi`, `os-prober` có mặt, `GRUB_DISABLE_OS_PROBER=false`.
>
> **Vẫn còn hai điều kiện trước khi chạm máy thật:**
> 1. Chờ bản **0.1.19 phát hành** (0.1.18 chưa có feature này).
> 2. Đang **thiếu đúng một phép thử**: boot từ đĩa đã cài trong VM xem menu
>    GRUB hiện và Arch vào được. Nên chạy trước ở VM thật hơn là test ngay
>    trên `nvme` — Windows thật có thêm bootmgr/BitLocker/secure-boot mà VM
>    chưa mô phỏng hết.

> Mục tiêu: có một hệ Arch thật để boot thử, mà **không** mất EndeavourOS,
> không mất Storage và không phải chia lại ổ NVMe.

## Tình trạng đĩa hiện tại

```
sda  119G
├─sda1  49G  ext4  "data"      ← chỗ trống, dùng để cài Arch test
├─sda2   1G  vfat  /boot/efi   ← ESP của EndeavourOS
└─sda3  69G  ext4  /           ← EndeavourOS

nvme0n1  477G
├─nvme0n1p1  100M vfat         ← ESP cũ (Windows), còn rác Microsoft
└─nvme0n1p2  469G ext4 Storage ← nơi chứa config/app/ISO
```

**Không chia lại `nvme0n1p2`.** Nó đang được 31 symlink dùng; thu nhỏ ext4 phải
unmount, tức là boot từ USB live. Rủi ro cao, lợi ích bằng 0 vì đã có `sda1`.

---

## Cách A — Test camareLOL đúng nghĩa (khuyến nghị)

camareLOL cần **một ổ trống**, không phải một phân vùng. Dùng USB ≥ 16G:

```bash
# 1. Ghi Arch ISO ra USB (USB sẽ bị xoá sạch)
sudo dd if=archlinux-x86_64.iso of=/dev/sdX bs=4M status=progress oflag=sync

# 2. Boot USB, chọn "Arch Linux install medium"

# 3. Trong live ISO, tải camareLOL
curl -fsSL https://raw.githubusercontent.com/tamkogay9876-code/camareLOL/main/scripts/camarelol.sh | bash

# 4. Chạy thử không ghi gì trước
camarelol --preview

# 5. Cài thật vào USB thứ hai (không phải USB đang boot)
camarelol
```

Kết quả cần thấy: 12/12 phase completed, reboot vào được greetd/Hyprland.

---

## Cách B — Cài Arch tay vào `/dev/sda1` (dual-boot thật)

Chỉ làm khi `sda1` **không còn dữ liệu cần**. Kiểm tra trước:

```bash
lsblk -f /dev/sda1          # phải là ext4, label data
sudo mkdir -p /mnt/archtest
sudo mount /dev/sda1 /mnt/archtest
ls -A /mnt/archtest | head  # phải rỗng; có gì thì backup trước
sudo umount /mnt/archtest
```

### B1. Boot Arch ISO, kết nối mạng

```bash
iwctl                      # nếu dùng Wi-Fi
ping -c1 archlinux.org
```

### B2. Format và mount

```bash
sudo mkfs.ext4 -L archtest /dev/sda1
sudo mount /dev/sda1 /mnt
```

### B3. Cài hệ cơ bản

```bash
sudo pacstrap -K /mnt base linux linux-firmware nano sudo networkmanager
sudo genfstab -U /mnt >> /mnt/etc/fstab
```

### B4. Vào chroot

```bash
sudo arch-chroot /mnt
ln -sf /usr/share/zoneinfo/Asia/Ho_Chi_Minh /etc/localtime
hwclock --systohc
echo "archtest" > /etc/hostname
echo "en_US.UTF-8 UTF-8" >> /etc/locale.gen
locale-gen
passwd root
systemctl enable NetworkManager
```

### B5. Cài bootloader vào ESP **của EndeavourOS** (không format ESP)

```bash
pacman -S --needed grub efibootmgr
mkdir -p /boot/efi
mount /dev/sda2 /boot/efi
grub-install --target=x86_64-efi --efi-directory=/boot/efi --bootloader-id=archtest
grub-mkconfig -o /boot/grub/grub.cfg
```

`--bootloader-id=archtest` tạo thư mục riêng `/boot/efi/EFI/archtest/`,
**không ghi đè** loader của EndeavourOS.

> ⚠️ Nếu `grub-install` báo ESP đã có GRUB của EndeavourOS, vẫn an toàn: nó chỉ
> thêm thư mục mới và một entry `Boot####` mới. Tuyệt đối **không** chạy
> `grub-install` mà không có `--bootloader-id`, và **không** format `/dev/sda2`.

### B6. Thoát và reboot

```bash
exit
sudo umount -R /mnt
reboot
```

Trong firmware (F11 trên MSI B450M-A PRO MAX II) sẽ có thêm entry **archtest**.

---

## Cách C — VM (nhanh nhất, không rủi ro)

```bash
VBoxManage createmedium disk --filename "$HOME/VirtualBox VMs/archtest.vdi" --size 16384
VBoxManage createvm --name archtest --ostype ArchLinux_64 --register
VBoxManage modifyvm archtest --memory 4096 --cpus 2 --boot1 dvd --boot2 disk
VBoxManage storagectl archtest --name SATA --add sata
VBoxManage storageattach archtest --storagectl SATA --port 0 --device 0 \
    --type hdd --medium "$HOME/VirtualBox VMs/archtest.vdi"
VBoxManage storageattach archtest --storagectl IDE --port 0 --device 0 \
    --type dvddrive --medium "$HOME/Tài liệu/carameLOL/.test/archlinux-x86_64.iso"
VBoxManage startvm archtest
```

Trong VM, chạy `camareLOL --preview` rồi cài thật vào `/dev/sda` của VM.

---

## Sau khi boot được — kiểm tra

```bash
cat /etc/os-release          # ID=arch
findmnt /                    # /dev/sda1 hoặc /dev/...
systemctl --failed           # phải rỗng
ip a                         # có mạng
```

Với camareLOL:

```bash
camarelol --self-test
journalctl -b -u greetd      # nếu chọn greetd
```

---

## Cách quay lại, xoá bản Arch test

Từ EndeavourOS hoặc live ISO:

```bash
# xoá entry firmware
sudo efibootmgr                 # tìm Boot#### "archtest"
sudo efibootmgr -b XXXX -B

# xoá loader
sudo mount /dev/sda2 /boot/efi
sudo rm -rf /boot/efi/EFI/archtest

# format lại sda1 thành kho dữ liệu
sudo mkfs.ext4 -L data /dev/sda1
```

EndeavourOS và Storage **không bị ảnh hưởng** trong toàn bộ quy trình này.

---

## Điều tuyệt đối không làm

| Không làm | Hậu quả |
|---|---|
| `mkfs` lên `/dev/sda2` | mất boot cả EndeavourOS lẫn Arch test |
| `mkfs` lên `/dev/sda3` | mất EndeavourOS |
| Chia lại `/dev/nvme0n1p2` | mất 17G config/app, 31 symlink gãy |
| Chạy camareLOL rồi chọn `/dev/sda` | camareLOL xoá **cả ổ**, mất EndeavourOS |
| `grub-install` không có `--bootloader-id` | có thể ghi đè loader EndeavourOS |

---

## Tóm lại

- Muốn test **camareLOL**: dùng **USB** hoặc **VM** (Cách A/C).
- Muốn test **dual-boot thật**: dùng **sda1** (Cách B), giữ nguyên ESP và
  không đụng Storage.
- **Không** chia lại Storage. Không có lợi ích, chỉ có rủi ro.
