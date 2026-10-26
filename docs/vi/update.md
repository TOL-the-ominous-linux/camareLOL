# Cập nhật camareLOL

[← Trang chủ tài liệu](index.md)

camareLOL không tự cập nhật.

---

## Không có trình cập nhật

Không có cờ `--update`. Không có mã chỉ mục. Trong binary không có đường nào
thay chính nó.

Đây là quyết định, không phải phần chưa làm. camareLOL xoá ổ đĩa. Một thành
phần việc của nó là thay thế chính nó là con đường ngắn nhất từ máy chủ bị
chiếm tới dữ liệu bị xoá — đối chiếu SHA-256 bảo vệ bạn khỏi một tệp *tải về*
bị hỏng hoặc bị thay, chứ không bảo vệ khỏi thứ khác. Điểm tin cậy lại là một
máy chủ mà đúng theo định nghĩa, kẻ tấn công cũng tới được.
Cơ chế đứng bên dưới cũng là một gánh nặng. Trước khi bị bỏ, mỗi bản phát
hành đi kèm một khoá ký, một chỉ mục cập nhật, một danh sách mirror và hai máy
chủ mirror. Chuỗi đó hỏng bốn lần trong một ngày. Kiểu hỏng luôn giống nhau: một
máy chủ ngừng trả lời, một máy khác vẫn phục vụ bản chỉ mục đóng băng, và mọi
bản camareLOL ngoài đời đều báo "bạn đang dùng bản mới nhất".

**Một mirror nói dối về độ mới còn tệ hơn không có mirror.**

---

## Thay vào đó, cập nhật thế này

camareLOL không có gói cho `pacman`, và cũng không có gói AUR. Chạy lại
dòng lệnh một dòng ở trên; nó tự lấy bản **ổn định** mới nhất rồi kiểm tra:

```console
$ curl -fsSL https://raw.githubusercontent.com/tamkogay9876-code/camareLOL/main/scripts/camarelol.sh | bash
```

Bản 1.0.0 là beta, phát hành dưới dạng GitHub pre-release. Ghim tag hoặc chọn
kênh beta:

```console
$ curl -fsSL https://raw.githubusercontent.com/tamkogay9876-code/camareLOL/main/scripts/camarelol.sh \
    | CAMARELOL_VERSION=v1.0.0 bash
```

```console
$ curl -fsSL https://raw.githubusercontent.com/tamkogay9876-code/camareLOL/main/scripts/camarelol.sh \
    | CAMARELOL_CHANNEL=beta bash
```

### Tải tay

Tải bản bạn muốn từ
[Releases](https://github.com/tamkogay9876-code/camareLOL/releases) rồi thay
binary:

```console
$ sudo pacman -S wget                 # nếu máy chưa có
$ sudo wget -O /usr/local/bin/camarelol \
    https://github.com/tamkogay9876-code/camareLOL/releases/download/v1.0.0/camarelol
$ sudo wget -O /tmp/SHA256SUMS \
    https://github.com/tamkogay9876-code/camareLOL/releases/download/v1.0.0/SHA256SUMS
$ cd /usr/local/bin && grep ' camarelol$' /tmp/SHA256SUMS | sha256sum -c -
$ sudo chmod +x /usr/local/bin/camarelol
$ camarelol --version                  # xem mình đang có bản nào
```

Bản băm phát hiện tệp bị hỏng hoặc bị thay khi tải. Nó không phải chữ ký:
tệp `SHA256SUMS` nằm cạnh binary, nên một máy chủ bị chiếm có thể thay cả hai.
`--version` cho bạn biết đang chạy bản phát hành nào.

### Nếu máy chỉ có Wi-Fi

camareLOL cần mạng để tải gói Arch. Nếu bạn cài bằng `pacman` hoặc tải tay thì
phải nối Wi-Fi trước bằng `iwctl` — công cụ có sẵn trong Arch live ISO:

```console
$ iwctl
[iwd]# device list
[iwd]# station wlan0 scan
[iwd]# station wlan0 get-networks
[iwd]# station wlan0 connect "TenMang"
[iwd]# station wlan0 enable
```

Mạng có mật khẩu thì `connect` sẽ hỏi, gõ mật khẩu vào. Tên giao diện không
phải lúc nào cũng là `wlan0` — xem `device list` để biết đúng tên.

Nếu `iwctl` không có:

```console
$ sudo pacman -S iwd
```

Hai dấu hiệu cho biết camareLOL đã thấy đủ mạng để tải gói: ô trạng thái ở
màn hình chào ghi `internet connection OK`, và `zram` trong danh sách dịch vụ
không bị gạch chéo.

Còn một trường hợp camareLOL tự nói trước: máy **chỉ có Wi-Fi, chưa nối, và
không có `iwctl` lẫn `nmtui`** — tức là không có công cụ nào để nối được. Lúc
đó màn hình chào hiện dòng cảnh báo vàng, thay vì chỉ ghi "no network
connection detected" rồi im lặng. Nếu `iwctl` còn có (live ISO luôn có), thì
bạn tự nối được, nên không cảnh báo.

### Qua npm

Cần `node` và `npm` trên máy:

```console
$ sudo pacman -S --needed nodejs npm curl
$ npm install -g @wenker-ai/camarelo
```

Vỏ npm tự hỏi nối Wi-Fi trước khi chạy trình cài — kể cả mạng có mật khẩu.

Trên **archiso** (live ISO) phải khác một chút: cơ sở dữ liệu pacman còn rỗng
nên `pacman -S` báo `database file for 'core' does not exist`. Kéo nó về rồi
cài trong cùng một lượt, kèm mạng đã nối — pacman cần mạng để tải DB, còn
Wi-Fi thì tùy cách cài:

- **Cài qua npm** (`npm install -g @wenker-ai/camarelo`): vỏ npm hỏi nối Wi-Fi
  bằng `iwctl` trước khi chạy trình cài, kể cả mạng có mật khẩu.
- **Cài bằng `pacman` hoặc tải tay**: phải tự nối Wi-Fi trước bằng `iwctl`
  (xem [mục Wi-Fi](#nếu-máy-chỉ-có-wi-fi)).

```console

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

## Kiểm tra bản cài của bạn: `--postcheck`

Thay vì tự đoán, hãy hỏi chính trình cài:

```console
$ sudo camarelol --postcheck
  PASS zram_unit
  PASS zram_wants
  PASS zram_bin
  PASS zram_conf
  PASS sshd_unit
  PASS sshd_wants
  PASS openssh
  all checks passed
```

Bảy dòng đó là bảy thứ phải **có thật trên đĩa**, không phải bảy lựa chọn
trong menu. Hệ thống bạn cài đĩa ở chỗ khác thì chỉ vào thư mục gốc của nó:

```console
$ sudo camarelol --postcheck /mnt
```

Có `FAIL` thì đó là thứ đã chọn nhưng không có thật — mà mục này không sửa
gì cả, chỉ nói thật. Sửa tay phần đã có sẵn phía trên, hoặc cài lại bằng bản
mới hơn.

### 0.1.13 có một lỗi: zram chưa được bật

Bản 0.1.13 viết đủ tệp cấu hình, nhưng **quên tạo symlink bật service**
(`systemctl enable` cho unit mẫu trong môi trường cài không sinh ra symlink
nào). Kết quả: `zram_conf` có mà `zram_wants` không, nên zram vẫn không chạy
dù cài đặt báo `completed`. Sửa tay:

```console
$ sudo ln -s ../../../../usr/lib/systemd/system/systemd-zram-setup@.service \
    /etc/systemd/system/multi-user.target.wants/systemd-zram-setup@zram0.service
$ sudo systemctl start systemd-zram-setup@zram0
$ zramctl
```

Bản sửa nằm trong kho và bài kiểm tra VirtualBox giờ chặn đúng chỗ này:
bước cuối mở đĩa đích lên và đòi cả bảy thứ có mặt, nên bản phát hành sau
này không thể bỏ sót nó.

---

## Xem mình đang có gì

```console
$ camarelol --version
camareLOL 1.0.0 (tag v1.0.0, commit abc1234)
```

Ba phần, và chúng nên khớp nhau:

| Phần | Ý nghĩa |
|---|---|
| `1.0.0` | bản phát hành binary được build từ |
| `tag v1.0.0` | thẻ git, cũng là tên bản phát hành |
| `commit abc1234` | đúng commit đó |

Nếu nó ghi `[built from a dirty working tree]`, binary đó được biên dịch từ thay
đổi chưa commit. Nó không phải bản phát hành, dù tự khai phiên bản gì.

---

## Không có gì khác thay đổi

Bỏ trình cập nhật chỉ xoá đúng phần code thay thế binary. Trình cài đặt, cổng an
toàn, giao thức IPC và tài liệu không hề bị ảnh hưởng. Xem
[những gì chưa làm](roadmap.md) để biết dự án đang đi đâu.
