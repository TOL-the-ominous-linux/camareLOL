# Cập nhật camareLOL

[← Trang chủ tài liệu](index.md)

camareLOL không tự cập nhật.

---

## Không có trình cập nhật

Không có cờ `--update`. Không có mã chỉ mục. Trong binary không có đường nào
thay chính nó.

Đây là quyết định, không phải phần chưa làm. camareLOL xoá ổ đĩa. Một thành
phần việc của nó là thay thế chính nó là con đường ngắn nhất từ máy chủ bị
chiếm tới dữ liệu bị xoá — kiểm tra chữ ký bảo vệ bạn khỏi một tệp *tải về*
bị sửa, chứ không bảo vệ khỏi thứ khác. Điểm tin cậy lại là một máy chủ mà
đúng theo định nghĩa, kẻ tấn công cũng tới được.

Cơ chế đứng bên dưới cũng là một gánh nặng. Trước khi bị bỏ, mỗi bản phát
hành đi kèm một khoá ký, một chỉ mục cập nhật, một danh sách mirror và hai máy
chủ mirror. Chuỗi đó hỏng bốn lần trong một ngày. Kiểu hỏng luôn giống nhau: một
máy chủ ngừng trả lời, một máy khác vẫn phục vụ bản chỉ mục đóng băng, và mọi
bản camareLOL ngoài đời đều báo "bạn đang dùng bản mới nhất".

**Một mirror nói dối về độ mới còn tệ hơn không có mirror.**

---

## Thay vào đó, cập nhật thế này

### Từ kho phần mềm hoặc AUR

Cách thông thường, giống mọi gói Arch khác:

```console
$ sudo pacman -S camarelol
```

### Tải tay

Tải bản bạn muốn từ
[Releases](https://github.com/tamkogay9876-code/camareLOL/releases) rồi thay
binary:

```console
$ sudo pacman -S wget                 # nếu máy chưa có
$ sudo wget -O /usr/local/bin/camarelol \
    https://github.com/tamkogay9876-code/camareLOL/releases/latest/download/camarelol
$ sudo chmod +x /usr/local/bin/camarelol
$ camarelol --version                  # xem mình đang có bản nào
```

Không có chữ ký để xác minh, vì cũng không có gì để xác minh. `--version` nói
cho bạn biết đang chạy bản phát hành nào, và đó là toàn bộ câu chuyện về niềm
tin.

### Qua npm

Cần `node` và `npm` trên máy:

```console
$ sudo pacman -S --needed nodejs npm curl
$ npm install -g @wenker-ai/camarelo
```

Trên **archiso** (live ISO) phải khác một chút: cơ sở dữ liệu pacman còn rỗng
nên `pacman -S` báo `database file for 'core' does not exist`. Kéo nó về rồi
cài trong cùng một lượt, kèm mạng đã nối (Wi-Fi bên ngoài quy trình này —
pacman cần mạng để tải DB):

```console
$ sudo pacman -Sy --needed nodejs npm curl
```

Vỏ npm không nằm trong binary: nó là mã Node chạy **trước khi** binary bắt đầu,
và binary vẫn không có đường nào thay chính nó. Khi bản trong gói cũ hơn bản
mới nhất trên GitHub Releases, vỏ hỏi một câu — trả lời `y` thì nó tải bản đó,
đối chiếu `SHA256SUMS`, thay binary trong gói và chạy tiếp; trả lời `n` thì đi
thẳng vào trình cài đặt.

Niềm tin đúng bằng tải tay: tệp kiểm nằm cùng một chỗ với binary, nên nó bắt
được tệp tải về hỏng giữa đường chứ không bắt được máy chủ bị chiếm.

---

## Nếu bạn đang ở 0.1.10 hoặc cũ hơn

Những bản đó có sẵn trình cập nhật. Bấm `u` ở màn hình chào và nó sẽ hỏi một
địa chỉ không còn tồn tại, hoặc âm thầm báo thành công nhờ một chỉ mục đóng
băng.

Không có rủi ro gì: nó không thể cài bất cứ thứ gì, vì máy chủ nó hỏi đã biến
mất. Hãy tải bản hiện tại và thay binary như hướng dẫn trên. Các bản sau này
không còn phím `u`.

---

## Nếu hệ thống cài bằng 0.1.12 mà có chọn zram

0.1.12 bật đúng unit (`systemd-zram-setup@zram0.service`) và cài
`zram-generator` — vậy mà vẫn không có swap zram nào chạy được.
`zram-generator` đọc `/etc/systemd/zram-generator.conf` mới biết phải tạo
thiết bị nào; thiếu tệp này thì nó chạy rồi thôi, hệ thống khởi động không
hề có zram. 0.1.13 bỏ chỗ hụt đó: khi cài đặt, trình cài đặt tự viết tệp cấu
hình theo lựa chọn `zram_size_mb`.

Hệ thống cài bởi 0.1.12 có sẵn unit và gói, chỉ thiếu tệp cấu hình. Cách nhẹ
nhất là thêm tệp bằng tay, không cần cài lại:

```console
$ sudo tee /etc/systemd/zram-generator.conf >/dev/null <<'EOF'
[zram0]
zram-size = min(ram / 2, 4096)
EOF
$ sudo systemctl start systemd-zram-setup@zram0
$ zramctl
```

`min(ram / 2, 4096)` chính là dự phòng của trình cài đặt: nửa RAM, tối đa
4 GiB. Muốn mức khác thì ghi số MiB thường, ví dụ `zram-size = 2048`. Sau
lệnh trên, `zramctl` phải hiện được `zram0`; nếu không, khởi động lại rồi
xem lại một lần nữa.

---

## Xem mình đang có gì

```console
$ camarelol --version
camareLOL 0.1.12 (tag v0.1.12, commit abc1234)
```

Ba phần, và chúng nên khớp nhau:

| Phần | Ý nghĩa |
|---|---|
| `0.1.12` | bản phát hành binary được build từ |
| `tag v0.1.12` | thẻ git, cũng là tên bản phát hành |
| `commit abc1234` | đúng commit đó |

Nếu nó ghi `[built from a dirty working tree]`, binary đó được biên dịch từ thay
đổi chưa commit. Nó không phải bản phát hành, dù tự khai phiên bản gì.

---

## Không có gì khác thay đổi

Bỏ trình cập nhật chỉ xoá đúng phần code thay thế binary. Trình cài đặt, cổng an
toàn, giao thức IPC và tài liệu không hề bị ảnh hưởng. Xem
[những gì chưa làm](roadmap.md) để biết dự án đang đi đâu.
