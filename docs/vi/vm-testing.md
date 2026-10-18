<!-- Dịch từ docs/en/vm-testing.md. Sửa bản gốc rồi dịch lại; đừng sửa bản này. -->
# Tự chạy thử

[← Chỉ mục tài liệu](index.md)

Hai cách: **xem trước**, không ghi gì cả, và **cài thật**, ghi vào một đĩa ảo.
Cả hai đều cần VirtualBox.

---

## Cách 1 — Xem trước, không ghi gì

Hãy dùng cách này trước. Nó kiểm tra giao diện và phát hiện ổ đĩa trên một máy
thật mà không đụng tới gì cả.

```console
$ cd /path/to/camareLOL
$ bash scripts/vm-release-test.sh create    # VM: Arch ISO + hai đĩa rỗng
$ bash scripts/vm-release-test.sh boot      # chạy không màn hình; tải binary, chạy self-test
$ bash scripts/vm-release-test.sh verify    # chụp lại kết quả
```

Bên trong VM:

```console
$ camarelol --version
$ camarelol --scan          # JSON: model, số sê-ri, phân vùng, mức rủi ro
$ camarelol --self-test     # các bước kiểm không phá huỷ
$ camarelol                 # mở TUI; không gõ gì thì không ghi gì
```

`--self-test` an toàn để chạy trên máy thật. `--scan` cũng vậy. TUI chỉ ghi khi
bạn gõ cụm từ xác nhận.

Ảnh chụp màn hình nằm trong `/tmp/opencode/vm-shots/`.

---

## Cách 2 — Cài thật vào một đĩa ảo

> **Script này xoá dữ liệu trên đĩa ảo.** Chỉ dùng cho VM. Không trỏ nó vào một
> máy thật.

```console
$ cd /path/to/camareLOL
$ bash scripts/vm-test.sh create    # VM + đĩa rỗng 20 GiB + đĩa FAT chứa binary
$ bash scripts/vm-test.sh boot      # boot, tự gõ các lệnh cài đặt
$ bash scripts/vm-test.sh check     # chụp lại kết quả
$ bash scripts/vm-test.sh destroy   # xoá VM
```

Kết quả mong đợi: mọi giai đoạn hoàn tất, `/mnt/etc/fstab` hợp lệ, và có
`vmlinuz-linux` cùng `grub.cfg`.

---

## Tự tạo một VM

Nếu bạn muốn nhìn màn hình thật.

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
$ VBoxManage startvm lol            # KHÔNG headless — bạn muốn nhìn màn hình
```

Bên trong ISO sống:

```console
# mount -o remount,rw /          # a live ISO usually mounts read-only
# pacman -Sy --needed curl
# curl -fLO https://github.com/tamkogay9876-code/camareLOL/releases/latest/download/camarelol
# chmod +x camarelol
# ./camarelol --self-test
# sudo ./camarelol --allow-destructive    # add --execute to actually write
```

Mọi thứ camareLOL cần đã nằm sẵn trong ISO sống: `sfdisk`, `sgdisk`, `mkfs.*`,
`pacstrap`, `genfstab`, `arch-chroot`. Không cần cài thêm gì.

## Khi không có terminal

Chạy qua một pipe, từ `cron`, hoặc trong CI sẽ thất bại với một thông báo rõ
ràng thay vì một panic — TUI không có chỗ để vẽ:

```
camareLOL needs a terminal to draw its interface, and there is none here.

For unattended runs there is no interface to draw, so use the scripted path:
    camarelol --scripted <config.toml>
and add --allow-destructive --execute to actually write to a disk.
```

---

## Cần kiểm tra gì

| làm điều này | kết quả mong đợi |
| --- | --- |
| mở TUI, di chuyển bằng `↑↓` | hiện model, số sê-ri và mức rủi ro cho từng ổ đĩa |
| chọn một ổ đĩa, nhấn `F10` | liệt kê các thao tác phá huỷ và yêu cầu gõ `ERASE` |
| gõ sai cụm từ | bị từ chối, và **không có gì được ghi** |
| nhấn `F1` hai lần | lớp phủ trợ giúp mở rồi đóng lại |
| chạy một lần cài thật | mọi giai đoạn hoàn tất, `fstab` hợp lệ, GRUB được cài |
| rút dây mạng, rồi tiếp tục | báo lỗi mạng, không bao giờ báo thành công giả |

Hàng thứ ba là hàng quan trọng nhất. Những hàng còn lại chỉ là tiện lợi.

---

## Dọn dẹp

```console
$ bash scripts/vm-test.sh destroy
$ bash scripts/vm-release-test.sh destroy
$ rm -rf ~/VirtualBox\ VMs/camarelol-*
```

Đĩa ảo và ISO nằm trong thư mục `.test/` của dự án. Xoá VM không ảnh hưởng
tới máy chủ.

## Tiếp theo

- [Kiểm thử](testing.md)
- [Những gì chưa làm](roadmap.md)
