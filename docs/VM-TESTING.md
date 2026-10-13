# Test camareLOL trên máy ảo

Hai cách: **xem trước (an toàn)** và **cài thật (ghi đĩa ảo)**.

---

## Cách 1 — Xem trước, không ghi gì

Dùng để làm quen giao diện và kiểm tra phát hiện ổ đĩa.

```console
$ cd "/home/xogfredia-admin/Tài liệu/carameLOL"
$ bash scripts/vm-release-test.sh create    # tạo VM (Arch ISO + 2 ổ rỗng)
$ bash scripts/vm-release-test.sh boot      # boot headless, tải binary, chạy self-test
$ bash scripts/vm-release-test.sh verify    # chụp ảnh kết quả
```

Kiểm tra nhanh trong VM:

```console
$ camarelol --version
$ camarelol --scan          # JSON: model, serial, phân vùng, mức rủi ro
$ camarelol --self-test     # các bước kiểm tra không phá hủy
$ camarelol                  # mở TUI (không gõ cụm xác nhận thì không ghi gì)
```

Ảnh chụp màn hình nằm ở `/tmp/opencode/vm-shots/`.

---

## Cách 2 — Cài thật vào ổ ảo

⚠️ **Script này XÓA dữ liệu trên ổ ảo.** Chỉ chạy với VM, đừng trỏ vào máy thật.

```console
$ cd "/home/xogfredia-admin/Tài liệu/carameLOL"
$ bash scripts/vm-test.sh create    # VM + ổ 20 GiB trống + đĩa FAT chứa binary
$ bash scripts/vm-test.sh boot      # boot, gõ lệnh cài đặt tự động
$ bash scripts/vm-test.sh check     # ảnh kết quả
$ bash scripts/vm-test.sh destroy   # xóa VM
```

Kết quả mong đợi: 11/11 phase hoàn thành, `/mnt/etc/fstab` hợp lệ, có `vmlinuz-linux` và `grub.cfg`.

---

## Tự tạo VM thủ công (nếu muốn xem màn hình thật)

```console
$ VBoxManage createvm --name lol --ostype ArchLinux_64 --register
$ VBoxManage modifyvm lol --memory 4096 --cpus 4 --boot1 dvd --boot2 disk
$ VBoxManage storagectl lol --name SATA --add sata --controller IntelAhci
$ VBoxManage createmedium disk --filename ~/lol.vdi --size 20480 --format VDI
$ VBoxManage storageattach lol --storagectl SATA --port 0 --device 0 \
    --type hdd --medium ~/lol.vdi
$ VBoxManage storagectl lol --name IDE --add ide
$ VBoxManage storageattach lol --storagectl IDE --port 0 --device 0 \
    --type dvddrive --medium "$PWD/.test/archlinux-x86_64.iso"
$ VBoxManage startvm lol            # KHÔNG headless — muốn nhìn màn hình
```

Trong VM: tải binary rồi chạy TUI.

```console
# mount -o remount,rw /       # live ISO thường mount read-only
# pacman -Sy --needed curl
# curl -sSL -o /root/camarelol \
    https://github.com/tamkogay9876-code/camareLOL/releases/download/v0.1.0/camarelol
# chmod +x /root/camarelol
# /root/camarelol --self-test
# /root/camarelol --allow-destructive      # thêm --execute khi muốn ghi thật
```

Bên trong live ISO đã có sẵn: `sfdisk`, `sgdisk`, `mkfs.*`, `pacstrap`, `genfstab`,
`arch-chroot` — đủ để cài, không cần cài thêm gói nào.

---

## Nên test những gì

| Việc | Kỳ vọng |
|---|---|
| Mở TUI, bấm ↑↓ xem danh sách ổ | hiển thị model, serial, mức rủi ro |
| Chọn ổ → Review (F10) | liệt kê thao tác phá hủy, yêu cầu gõ `ERASE` |
| Gõ sai cụm từ xác nhận | bị từ chối, **không ghi gì** |
| Bấm `F1` rồi `F1` lần nữa | overlay trợ giúp đóng lại |
| Chạy install thật | 11 phase, `fstab` hợp lệ, GRUB được cài |
| Rút cáp mạng rồi bấm Enter | báo lỗi mạng, không báo "thành công" giả |

---

## Dọn dẹp

```console
$ bash scripts/vm-test.sh destroy
$ bash scripts/vm-release-test.sh destroy
$ rm -rf ~/VirtualBox\ VMs/camarelol-*
```

Ổ đĩa ảo và ISO nằm trong thư mục dự án (`.test/`), xóa VM không ảnh hưởng máy thật.