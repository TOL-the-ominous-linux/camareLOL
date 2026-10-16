# @wenker-ai/camarelo

camareLOL — trình cài đặt Arch Linux chạy trong terminal, ưu tiên an toàn.

Cài hệ điều hành mà không phải gõ hàng chục lệnh shell. Nó dò phần cứng, đề
xuất cách phân vùng an toàn, cho bạn xem chính xác nó sắp làm gì, và chỉ ghi
vào ổ đĩa sau khi bạn xác nhận.

**Nó không phải lớp bọc shell.** Trình cài đặt có quy trình riêng:
phát hiện → lập kế hoạch → xem lại → xác nhận → thực thi → kiểm tra, với một
cổng an toàn đứng trước mọi thao tác phá huỷ.

## Cài

Trên Arch, cần `node`, `npm`, `curl`, `git`, `sudo`:

```console
$ sudo pacman -S --needed nodejs npm curl git
$ npm install -g @wenker-ai/camarelo
```

(`npm i -g @wenker-ai/camarelo` cũng là một — `i` là tên ngắn của `install`.)

Trên **archiso** (live ISO) pacman có cơ sở dữ liệu rỗng — `pacman -S` riêng lẻ
sẽ báo `database file for 'core' does not exist`. Nối mạng trước rồi dùng:

```console
$ sudo pacman -Sy --needed nodejs npm curl git
```

Sau đó:

```console
$ camarelol --version      # xem đang có bản nào
$ camarelol                # mở giao diện
```

## Lần chạy đầu tiên hỏi hai câu

**1. Bản trên npm có thể cũ hơn GitHub Releases.**

Gói này đóng gói một bản binary tại thời điểm phát hành. Nếu GitHub đã có bản
mới, camareLOL nói ra và hỏi có cập nhật không:

```
═══════════════════════════════════════════════════════════
 Bản trên npm có thể cũ: 0.1.12, GitHub có 0.1.13

 Cập nhật không? (y/n)
═══════════════════════════════════════════════════════════
```

- **y** — tải bản mới từ GitHub Releases, kiểm SHA-256 so với tệp `SHA256SUMS`
  phát hành kèm, rồi cài đè.
- **n** — bỏ qua, đi tiếp.

Không có mạng thì không hỏi gì cả. Lúc đó người dùng cũng chưa nối được để cài
hệ thống.

**2. Nối Wi-Fi.**

Nếu máy chưa có mạng và có giao diện không dây, camareLOL dẫn qua:

```
=== Nối Wi-Fi ===
Giao diện không dây: wlan0

Các mạng đã lưu:
  1) Nha

Tên mạng (SSID):
```

Dùng `iwctl` — công cụ có sẵn trong Arch live ISO. Cho tối đa ba lần thử, vì
lần đầu thường là để nhập mật khẩu và lần sau mới nối được.

Sau đó vào màn hình cài đặt như bình thường.

## Về `postinstall`

Nó **không hỏi gì**. `npm install` thường chạy trong CI và Docker build, nơi
không có ai trả lời; một prompt chết đứng ở đó giết cả bài build. Nó chỉ kiểm
xem máy đủ dùng chưa và in ra điều cần cài.

## camareLOL không tự cập nhật

Điều này là của chương trình Rust, không phải của gói npm.

Trong binary không có mã nào thay chính nó, không có cờ `--update`, không có mã
chỉ mục. Chương trình này xoá ổ đĩa; một thành phần việc của nó là thay thế chính
nó là con đường ngắn nhất từ máy chủ bị chiếm tới dữ liệu bị xoá.

Cơ chế hỏi "có cập nhật không" nằm ở gói npm, chạy **trước khi** binary có mặt
trên đĩa, và cần bạn gõ `y`. Người dùng chọn tin vào GitHub Releases, không
phải vào máy chủ bên thứ ba.

Tài liệu đầy đủ: [docs/{en,vi,zh}/update.md](https://github.com/tamkogay9876-code/camareLOL/tree/main/docs)

## Yêu cầu

| | |
|---|---|
| Hệ điều hành | Arch Linux (hoặc dự án phái sinh) |
| Kiến trúc | x64 |
| Node | ≥ 18 (chỉ để chạy phần vỏ npm) |
| Công cụ | `curl`, `git`, `sudo`, `pacman` |
| Wi-Fi (tuỳ chọn) | `iwctl` — `sudo pacman -S iwd` |

## Kiểm tra toàn vẹn

Gói phát hành kèm tệp `SHA256SUMS`:

```console
$ curl -sSLO https://github.com/tamkogay9876-code/camareLOL/releases/latest/download/camarelol
$ curl -sSLO https://github.com/tamkogay9876-code/camareLOL/releases/latest/download/SHA256SUMS
$ sha256sum -c SHA256SUMS
```

## Giấy phép

GPL-3.0-or-later. Mã nguồn: <https://github.com/tamkogay9876-code/camareLOL>
