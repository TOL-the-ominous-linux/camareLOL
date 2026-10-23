# Hướng dẫn test dual-boot Arch không đụng Storage

> **Cập nhật 22 Oct 2026 — dual-boot đã chạy thật trong VM.**
> `scripts/vm-dualboot.sh` cài camareLOL cạnh một "Windows" giả bên trong VM
> `camarelol-dualboot`: bảng phân vùng ESP/MSR/C:/recovery dựng bằng sfdisk,
> ba tệp đánh dấu (bootmgr trên ESP, file data trên C:, marker trên recovery)
> **còn nguyên sau khi cài** (run 4 & run 7, log
> `/tmp/opencode/vm-dualboot-run{4,7}.log`; nguyên liệu lưu tại
> `/mnt/Storage/home-data/caramelol-evidence/dualboot-vm/`).
> root mới ở `sda5`, GRUB đặt lên ESP cũ (không format lại), fstab mount
> `/boot/efi`, `os-prober` có mặt, `GRUB_DISABLE_OS_PROBER=false`.
> Run 7 thêm: Windows giả mang cặp `bootmgfw.efi` + `BCD` (đúng điều kiện
> của probe `efi/20microsoft` của os-prober) và `grub.cfg` sinh ra có
> `menuentry "Windows Boot Manager (on /dev/sda1)"`. Nguyên nhân entry từng
> vắng mặt chốt bằng traced os-prober trong VM: **chỉ thiếu tệp BCD**, cùng
> một True bug ở mã nguồn — cờ os-prober từng được viết ở pha
> system-config, SAU pha bootloader sinh grub.cfg; nay pha bootloader tự
> viết (run 7 dùng binary `c314241` chứa bản sửa).
>
> Boot từ đĩa đã cài cũng đã chứng minh trong VM: NVRAM →
> `EFI/camareLOL/` trên ESP dùng lại → nhân Arch → initramfs → màn hình
> đăng nhập; ảnh: `grub-booting-arch.png`, `booted-to-login.png`,
> `getty-password-prompt.png` (cùng thư mục lưu trữ ở trên).
>
> **Điều kiện trước khi chạm máy thật còn lại một:** phát hành
> **0.1.19** (0.1.18 chưa có feature này). Còn những thứ chỉ Windows
> thật mới trả lời được — entry menu boot vào Windows thật thật sự, và
> thu nhỏ phân vùng từ bên trong Windows — nằm trong phần hoãn của
> roadmap; khi test máy thật vẫn **chỉ dùng sda1**, tuyệt đối không đụng
> `nvme` cho tới khi ánh xạ Windows thật được đọc thấy trong VM.

> **Cập nhật 23 Oct 2026 — 0.1.20 phát hành, điều kiện tròn đủ.**
> Bắt đầu bằng **Cách B2 (camareLOL dual-boot)** dưới đây; Cách B (thủ công)
> giữ lại làm phương án dự phòng.

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

## Cách A — Test camareLOL đúng nghĩa (USB khác, không đụng đĩa thật)

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

## Cách B2 — dual-boot máy thật bằng chính camareLOL 0.1.20 (khuyến nghị)

Chính là đường mà VM `camarelol-dualboot` đã chứng minh (run 4 & 7), giờ chạy
trên đĩa của máy này: camareLOL giữ nguyên EndeavourOS (sda3) và dùng lại
cùng ESP (sda2) như đã làm với Windows giả.

**Bố cục hiện tại và số phận từng vùng:**

```
sda  119G ← đĩa dual-boot
├─sda1  49G ext4 "archtest"  ← RỖNG (đã xác nhận 23 Oct) → xóa để tạo khe
├─sda2   1G vfat /boot/efi   ← ESP của EndeavourOS → camareLOL dùng lại
└─sda3  69G ext4 /           ← EndeavourOS, giữ nguyên (os-prober thêm entry)
nvme0n1                     ← Storage: KHÔNG đụng
```

Điểm mấu chốt: planner dual-boot cần một **khe trống (gap)** trên GPT, mà
camareLOL không bao giờ tự xoá partition có sẵn (safety-first). Xoá `sda1`
là việc tay làm trước, trong live ISO.

### B2-0. Chuẩn bị trên máy đang chạy (trước khi boot USB)

```bash
# 1. Xác nhận lần cuối sda1 rỗng — nếu in ra bất cứ gì, DỪNG và backup
sudo mkdir -p /mnt/check-sda1
sudo mount /dev/sda1 /mnt/check-sda1
ls -A /mnt/check-sda1
sudo umount /mnt/check-sda1

# 2. Ghi Arch ISO ra USB ≥ 2 GiB (USB sẽ bị xoá sạch; sdX = USB thật,
#    kiểm tra bằng lsblk — đừng nhầm với sda/nvme!)
sudo dd if="$HOME/Tài liệu/carameLOL/.test/archlinux-x86_64.iso" \
        of=/dev/sdX bs=4M status=progress oflag=sync
```

### B2-1. Boot USB live (F11 trên MSI B450M-A PRO MAX II)

```bash
iwctl                       # Wi-Fi nếu không có dây
ping -c1 archlinux.org      # mạng phải sống, pacstrap cần internet

# Tải bản phát hành mới nhất (0.1.20)
curl -fsSL https://raw.githubusercontent.com/tamkogay9876-code/camareLOL/main/scripts/camarelol.sh | bash
./camarelol -V              # phải in "camareLOL 0.1.20 (tag v0.1.20…)"

# Xóa sda1, mở khe cho planner
sgdisk --delete=1 /dev/sda
blockdev --rereadpt /dev/sda
parted /dev/sda print       # còn đúng 2 phân vùng: số 2 = ESP (vẫn là
                            # /dev/sda2) và số 3 = root EndeavourOS (sda3)
                            # ⚠️ GPT xoá entry không đổi số: sda2/sda3
                            # giữ nguyên cả số lẫn tên thiết bị!
```

Dùng `sgdisk --delete=1` xoá đúng **một entry** của GPT; hai entry còn lại
(số 2, số 3) nguyên vẹn — cả mã phân vùng lẫn tên thiết bị trong nhân
(`/dev/sda2`, `/dev/sda3`). Đây vẫn là tư thế "cùng ESP, cùng root cũ" mà
planner nhìn thấy.

### B2-2. camareLOL chọn chế độ dual-boot

```bash
./camarelol --preview       # ĐỌC TRƯỚC KHI LÀM
```

Ở DiskMode chọn **mục 3 — Dual-boot (giữ mọi phân vùng)**. Màn Review phải
cho thấy đúng ba điều VM từng chứng minh:

- sda-ESP **"keep existing"**, mount `/boot/efi`, `format=false` — không
  hề format lại ESP của EndeavourOS;
- root EndeavourOS **"keep existing"**, `format=false`;
- một hàng mới cho root của camareLOL đặt bằng `sgdisk --new=<số>:<start>`:
  **không có `--zap-all`** trong danh sách lệnh, không có lệnh nào bảo sfdisk
  viết lại bảng.

`reuse_esp` tự bật khi chọn mode 3 → planner biết ESP cũ và định vị khe trống.

Khớp kế hoạch thì mở TUI thật (`./camarelol`), chọn cùng mode 3, rồi
**Execute** — installer sẽ hỏi xác nhận destructive theo thường lệ. 12 pha
phải hoàn tất, như run 7 trong VM. 0.1.20 thêm cho bạn hai thứ VM đã dựng:
tệp `/etc/NetworkManager/conf.d/00-camarelol.conf` (`dhcp=internal`), và việc
dọn entry firmware `Boot####` chết (trỏ vào loader không còn trên ESP của
chính ta — như entry `archtest` cũ nếu còn).

### B2-3. Reboot

```bash
reboot
```

Lần này không cần F11 nữa: firmware menu sẽ có entry GRUB của camareLOL
(xem ở `Boot0004`/`Boot####` nào mới xuất hiện bằng `efibootmgr` từ live
ISO — 0.1.20 chỉ xoá entry trỏ vào tệp loader không còn, entry còn sống
đều được giữ).

Boot được là tới GRUB. Menu GRUB phải có ít nhất hai entry: hệ camareLOL
vừa cài và **EndeavourOS** (os-prober đã quét sda3). Ảnh nhãn như VM. Nếu
chọn EndeavourOS mà không boot nổi, DỪNG và chụp ảnh màn hình — entry
EndeavourOS của nó nằm ở mục `EFI/EndeavourOS(…)` trên cùng ESP, GRUB
chainload vào được.

Sau khi vào hệ mới, kiểm tra nhanh:

- `systemctl status NetworkManager` — enabled, có tệp
  `/etc/NetworkManager/conf.d/00-camarelol.conf`;
- `zramctl` — zram0 thật (theo cấu hình chọn khi cài);
- `efibootmgr -v` — đúng những entry còn sống; entry cũ trỏ file không có
  đã bị 0.1.20 dọn;
- menu GRUB chứa EndeavorOS → chọn thử, máy quay lại EndeavourOS nguyên vẹn.

---

## Cách B — cài Arch tay vào `/dev/sda1` (dự phòng)

Nếu planner camareLOL từ chối kế hoạch một cách chính đáng — khe quá nhỏ,
firmware BIOS, đĩa đang mounted, hay bất cứ gate nào — làm tay theo cách
cũ dưới đây. Toàn bộ chỉ với `sda1` đã xoá/khoát sẵn và ghi bootloader
với `--bootloader-id=archtest`: **không hề format ESP, không đụng ESP của
EndeavourOS.**

### B1. Vẫn trong live ISO — format và mount (khe trống từ B2-1)

```bash
iwctl                      # nếu dùng Wi-Fi
ping -c1 archlinux.org
mkfs.ext4 -L archtest /dev/sda1     # đĩa "khe trống" mới — bây giờ nó là
                                    # phân vùng số 1 theo print ở trên
mount /dev/sda1 /mnt
```

### B2. Cài hệ cơ bản

```bash
pacstrap -K /mnt base linux linux-firmware grub efibootmgr sudo \
    networkmanager openssh
genfstab -U /mnt >> /mnt/etc/fstab
arch-chroot /mnt
ln -sf /usr/share/zoneinfo/Asia/Ho_Chi_Minh /etc/localtime
hwclock --systohc
echo "archtest" > /etc/hostname
echo "en_US.UTF-8 UTF-8" >> /etc/locale.gen
locale-gen
passwd root
systemctl enable NetworkManager
```

### B3. Bootloader vào **ESP dùng chung với EndeavourOS** — không format

```bash
mkdir -p /boot/efi
mount /dev/sda2 /boot/efi          # ESP GỐC — mount lại, KHÔNG mkfs.vfat!
grub-install --target=x86_64-efi --efi-directory=/boot/efi \
             --bootloader-id=archtest
grub-mkconfig -o /boot/grub/grub.cfg
exit
umount -R /mnt
reboot
```

`--bootloader-id=archtest` tạo thư mục riêng `/boot/efi/EFI/archtest/` —
KHÔNG ghi đè loader của EndeavourOS. Câu chốt của VM: entry firmware chỉ
thêm, không ghi đè. F11 chọn "archtest" khi cần quay về.

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
