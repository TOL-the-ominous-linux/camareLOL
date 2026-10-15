<!-- Dịch từ docs/en/testing.md. Sửa bản gốc rồi dịch lại; đừng sửa bản này. -->
# Kiểm thử

[← Chỉ mục tài liệu](index.md)

Bốn lớp, mỗi lớp trả lời một câu hỏi khác nhau. Lớp quan trọng nhất là lớp cuối:
liệu nó có chạy được trên một máy thật sự phải khởi động hay không.

## 1. Kiểm thử đơn vị

```console
$ cargo test --workspace                        # Rust core + TUI
$ cd python && python3 -m unittest discover -s tests -v
$ cd java && ./build.sh                         # biên dịch, chạy CatalogTest, đóng gói jar
$ python site/test_markdown.py                  # bộ dựng trang
$ python site/test_render_site.py               # bản dựng trang, không mạng
$ python scripts/check_pkgbuild.py              # PKGBUILD khớp thẻ mới nhất
```

Nhanh, không ổ đĩa, không mạng.

## 2. Kịch bản nghiệm thu

Không cần ổ đĩa thật. Tiêu chí nghiệm thu trong đặc tả chạy trên các khảo sát
thiết bị tổng hợp:

```console
$ cd python && PYTHONPATH=. python3 -m camarelol_tools.testutil
PASS  empty-disk: self-test: OK
PASS  windows-disk: self-test: OK
PASS  partition-no-fs: self-test: OK
```

| kịch bản | điều nó phải chứng minh |
| --- | --- |
| ổ đĩa trống | một cách phân vùng đầy đủ có thể được lập kế hoạch và áp dụng |
| ổ đĩa Windows | các phân vùng ESP, recovery và dành riêng cho Microsoft được giữ nguyên, không bị định dạng lại |
| phân vùng không có hệ thống tệp | bị phát hiện là chưa định dạng thay vì mặc định là dùng được |
| đổi danh tính thiết bị | lượt chạy huỷ thay vì ghi vào một ổ đĩa khác |
| cấu hình khởi động không hợp lệ | bị từ chối lúc lập kế hoạch, không phải phát hiện giữa chừng |

Những kịch bản này chạy trên tệp nhị phân đã build với `--mock`, nên chúng kiểm
thử đường đi mã thật chứ không phải một bản mô phỏng của nó.

## 3. Kiểm thử toàn hệ thống trong VirtualBox

Bằng chứng thật. Khởi động một Arch ISO trong VM với đĩa ảo **trống** và chạy
trình cài đặt ở chế độ có kịch bản.

```console
$ ./scripts/vm-test.sh create      # VM + 20 GiB empty disk + Arch ISO + data disk
$ ./scripts/vm-test.sh boot        # headless boot, drives the VM with keystrokes
$ ./scripts/vm-test.sh check       # screenshot and verify
$ ./scripts/vm-test.sh destroy
```

Bên trong VM:

```console
# mount /dev/sdb /mnt
# /mnt/camarelol --scan > /tmp/survey.json
# python3 /mnt/scripts/vm-helper.py /tmp/survey.json /tmp/test-profile.toml
# /mnt/camarelol --scripted /tmp/test-profile.toml --allow-destructive --execute
```

### Thành công trông như thế nào

```
preflight      completed  firmware=BIOS bootloader=GRUB fs=ext4 partitions=2
partition      completed  wrote GPT layout to /dev/sda
format         completed  filesystems created
mount          completed  target mounted
pacstrap       completed  installed 16 packages
fstab          completed  fstab generated
bootloader     completed  GRUB installed
system-config  completed  locale, keymap, timezone and hostname written
accounts       completed  0 user(s) configured
services       completed  services enabled
```

Được xác minh sau đó bên trong VM:

```console
# lsblk -f /dev/sda
sda1  vfat  LABEL=CAMARELOL                       (ESP)
sda2  ext4  LABEL=camarelol-root   mounted on /mnt
# cat /mnt/etc/fstab
/dev/sda2  /     ext4  defaults 0 2
# ls /mnt/boot
vmlinuz-linux  grub/  grub.cfg  i386-pc/
# ls /mnt/usr/bin/pacman
```

### Điều đó chứng minh

- tệp nhị phân chạy được trong môi trường Arch sống, không thiếu thư viện nào;
- ổ đĩa trống được phát hiện và một cách phân vùng GPT được lập kế hoạch;
- bảng phân vùng được ghi và các hệ thống tệp được tạo;
- pacstrap cài được một hệ thống nền;
- kết quả `genfstab` hợp lệ và GRUB được cài kèm các module BIOS của nó;
- hệ thống đích có một nhân, một trình nạp khởi động và một trình quản lý gói.

Một bộ kiểm thử đơn vị xanh không nói được điều gì trong số đó.

### Những lỗi mà nó bắt được

Tất cả những lỗi dưới đây đều vượt qua mọi kiểm thử đơn vị.

1. Script `sfdisk` sinh ra yêu cầu nhiều sector hơn sức chứa của đĩa — các khoảng
   trống căn chỉnh và header backup của GPT không được tính vào.
2. Phân vùng BIOS boot 1 MiB bị âm thầm bỏ mất, vì bộ sinh dừng ở một phân vùng
   nhỏ hơn bước căn chỉnh.
3. Nhân không bao giờ được yêu cầu đọc lại bảng phân vùng trước `mkfs`.
4. Thao tác mount bị ràng buộc dưới thao tác pacstrap nhưng lại được xác nhận ở
   một mức rủi ro khác, nên các lượt chạy dừng ở bước `mount`.
5. `grub` thiếu khỏi danh sách pacstrap, nên `grub-install` không chạy được bên
   trong hệ thống đích.
6. `zram-config` không phải là một unit thật; chỉ một unit thiếu đã huỷ cả giai
   đoạn service.

## 4. Đóng gói

CI build tệp tarball phát hành và chạy `makepkg` trên nó bên trong một container
Arch. Một `PKGBUILD` không build được sẽ làm build thất bại thay vì chờ ai đó
báo cáo.

Phần này tồn tại vì công thức từng ghi `pkgver=0.1.3` trong khi bản phát hành là
`0.1.10`, với một `source` trỏ tới một tarball chưa từng được công bố.

## Những gì không được kiểm thử

| không phủ | vì sao |
| --- | --- |
| Dual-boot với Windows thật | cần giấy phép Windows; kịch bản mô phỏng đã phủ logic phát hiện |
| Thu nhỏ NTFS | cố ý hoãn lại cho một bản phát hành sau |
| Build AUR | cần mạng và tương tác người dùng |

## Tự chạy thử

[Tự chạy thử](vm-testing.md) có hướng dẫn thực hành, gồm cả chế độ xem trước
không ghi gì cả.

## Tiếp theo

- [Kiến trúc](architecture.md)
- [Tự chạy thử](vm-testing.md)
