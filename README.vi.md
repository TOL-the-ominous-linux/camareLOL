<p align="center">
  <img src="assets/camareLOL.png" alt="camareLOL" width="140">
</p>

# camareLOL

[![m0QpBjbzyd](https://yourimageshare.com/ib/m0QpBjbzyd.gif)](https://yourimageshare.com/ib/m0QpBjbzyd)

> Trình cài đặt Arch Linux chạy trong terminal, ưu tiên an toàn — có linh hồn Việt.

[Tiếng Anh](README.md) · **Tiếng Việt** · [中文](docs/zh/index.md)

> ## ⚠ ALPHA — đã qua nhiều bản cập nhật, vẫn còn lỗi
>
> camareLOL ở giai đoạn **demo/alpha**. Đã có 17 bản phát hành, nhưng số bản
> **không** phản ánh độ ổn định. Những lỗi như tên gói Arch viết sai chỉ lộ
> ra khi chạy thật trong máy ảo — và chúng đã làm hỏng cả bản cài:
>
> - `greetd-tuigreeter` thay vì `greetd-tuigreet` → **Hyprland và Sway không
>   cài được**
> - `p7zip` thay vì `7zip` → đã bị gỡ khỏi kho Arch
> - yêu cầu cài gói `systemd-boot` không tồn tại → **mọi** bản cài chọn
>   systemd-boot đều chết ở `pacstrap`
>
> Cả ba đều qua được mọi unit test. Chúng chỉ chết trong VirtualBox.
>
> **Vì vậy:**
>
> 1. Chỉ chạy trên **ổ trống** — ổ đang có Windows hay dữ liệu thì đừng.
> 2. Chạy từ **Arch live ISO**, không phải trên máy bạn đang dùng.
> 3. Sao lưu mọi thứ trước khi cài bất cứ thứ gì.
>
> Số phiên bản giữ dạng `0.1.x` là có chủ ý. `0.x` theo SemVer nghĩa là
> "chưa ổn định". Đặt thành `5.6` sẽ khiến người khác tin rằng đây là phần
> mềm đã bảo trì lâu và chạy ổn định — một thông đoạp sai, và nguy hiểm với
> một công cụ xoá ổ đĩa.

camareLOL cài Arch Linux từ terminal mà không bắt bạn gõ hàng chục lệnh
shell. Nó dò phần cứng, đề xuất cách phân vùng an toàn, cho bạn xem chính
xác nó sắp làm gì, và chỉ ghi vào ổ đĩa sau khi bạn xác nhận.

**Nó không phải lớp bọc shell.** Trình cài đặt có quy trình riêng: phát hiện →
lập kế hoạch → xem lại → xác nhận → thực thi → kiểm tra, với một cổng an
toàn đứng trước mọi thao tác phá huỷ.

| | |
|---|---|
| Phát hành | <https://github.com/tamkogay9876-code/camareLOL/releases> |
| Thảo luận | <https://github.com/tamkogay9876-code/camareLOL/discussions> |
| Vấn đề | <https://github.com/tamkogay9876-code/camareLOL/issues> |

## Tài liệu

Viết bằng ba ngôn ngữ. Mỗi trang có bảng trỏ sang hai ngôn ngữ còn lại.

| Chủ đề | Tiếng Việt | English | 中文 |
|---|---|---|---|
| Tổng quan | [index](docs/vi/index.md) | [EN](docs/en/index.md) | [ZH](docs/zh/index.md) |
| Ba thành phần | [architecture](docs/vi/architecture.md) | [EN](docs/en/architecture.md) | [ZH](docs/zh/architecture.md) |
| Giao thức IPC | [ipc-protocol](docs/vi/ipc-protocol.md) | [EN](docs/en/ipc-protocol.md) | [ZH](docs/zh/ipc-protocol.md) |
| Được kiểm tra gì | [testing](docs/vi/testing.md) | [EN](docs/en/testing.md) | [ZH](docs/zh/testing.md) |
| Tự chạy thử | [vm-testing](docs/vi/vm-testing.md) | [EN](docs/en/vm-testing.md) | [ZH](docs/zh/vm-testing.md) |
| Cập nhật | [update](docs/vi/update.md) | [EN](docs/en/update.md) | [ZH](docs/zh/update.md) |
| Chưa làm gì | [roadmap](docs/vi/roadmap.md) | [EN](docs/en/roadmap.md) | [ZH](docs/zh/roadmap.md) |

Bắt đầu với [tổng quan](docs/vi/index.md).

## Cài đặt

### Một dòng (cầu nối)

Cách nhanh nhất, và không thể cài nhầm thứ:

```console
$ curl -fsSL https://raw.githubusercontent.com/tamkogay9876-code/camareLOL/main/scripts/camarelol.sh | bash
```

Nó tự lấy bản phát hành mới nhất, **kiểm tra ba lớp** — mã HTTP, ELF magic,
SHA-256 — rồi trao quyền kiểm soát lại cho bạn. Lớp kiểm thứ ba không phải
để trang trí:

```console
$ curl -sSLO https://github.com/.../camarelol    # thiếu -f và -L
$ chmod +x camarelol
$ ./camarelol --self-test
./camarelol: line 1: Not: command not found
```

Thiếu `-f`, curl lưu trang 404 của một proxy vào tệp tên `camarelol` rồi vẫn
trả mã 0. `chmod +x` vẫn thành công trên một tệp văn bản. Bash cố chạy nó
như một kịch bản shell. Một trình cài đặt xoá ổ đĩa không được phép bảo bạn
"xong" khi thứ nó tải về là HTML.

### Từ bản phát hành

Tải binary từ [Releases](https://github.com/tamkogay9876-code/camareLOL/releases)
rồi chạy từ Arch live ISO:

```console
$ chmod +x camarelol
$ ./camarelol --self-test      # kiểm không phá huỷ; an toàn trên máy thật
$ sudo ./camarelol             # mở TUI
```

### Qua npm

```console
$ sudo pacman -S --needed nodejs npm curl
$ npm install -g @wenker-ai/camarelo
```

Trên **archiso** (live ISO) cơ sở dữ liệu pacman rỗng — dùng
`sudo pacman -Sy --needed nodejs npm curl` sau khi đã nối mạng.

### Từ mã nguồn

```console
$ ./install.sh                # build rồi cài vào /usr/local/bin
$ ./build.sh build            # chỉ build, không cài
$ ./build.sh test             # unit test + kịch bản chấp nhận
$ ./build.sh lint             # clippy + rustfmt
```

## Cài thật cần gì

Ba điều kiện, thiếu một là không cài được:

```console
$ sudo ./camarelol --allow-destructive --execute
```

- `--allow-destructive` mở cổng an toàn
- `--execute` cho phép ghi thật (thiếu thì chỉ chạy thử trong RAM)
- ở màn **Review**, gõ cụm xác nhận: `ERASE` (rủi ro thấp/trung bình) hoặc
  `ERASE AND WIPE` (rủi ro cao). Không phân biệt hoa thường.

Gõ sai thì bị từ chối và **không ghi gì cả**.

## Xem trước: `--preview`

```console
$ sudo ./camarelol --preview
```

Chạy đủ 12 pha và in ra **từng lệnh thật** sẽ chạy (`sgdisk`, `sfdisk`, `mkfs`,
`pacstrap`, `genfstab`, `arch-chroot`, `mkinitcpio`, …) — nhưng không lệnh nào
được chạy và không byte nào được ghi vào đĩa. `--allow-destructive` và
`--execute` bị bỏ qua khi có `--preview`, nên `--preview --execute` vẫn chỉ là
xem trước. Dải cảnh báo đỏ hiện trên mọi màn hình suốt lần chạy.

Không cần giao diện:

```console
$ sudo camarelol --scripted <profile.toml> --allow-destructive --execute
```

## Nó làm được gì hôm nay

- Dò ổ đĩa, phân vùng, hệ thống tệp, ESP, Windows, phân vùng recovery và
  Microsoft-reserved, các mount của hệ đang chạy.
- Lên kế hoạch GPT/MBR: ESP, BIOS boot, `/`, `/home`, swap — chọn ext4, btrfs
  hoặc xfs, và kiểm tra theo firmware.
- Phát hiện Windows và **không chạm vào nó**.
- Cài Arch tối thiểu: pacstrap, fstab, bootloader (GRUB/systemd-boot), locale,
  keymap, timezone, hostname, user, sudo, service, initramfs.
- Cổng an toàn: khóa sẵn. Cần `--allow-destructive` + cụm xác nhận.
- Cấu hình lưu/đọc, có số phiên bản, **không chứa mật khẩu**.
- Ghi nhật ký mọi pha, có che bí mật.

## Cố ý chưa làm gì

- Xoá đĩa chỉ vì bạn chọn nó trong menu.
- Cài gói AUR khi chưa có đồng ý rõ ràng và một bản build bởi người dùng thường.
- Thu nhỏ NTFS — tiêu chí chấp nhận đã viết, tính năng để dành.
- Dùng lại ESP có sẵn cho dual-boot.

## Kiến trúc

Ba thành phần, một giao thức:

| thành phần | vai trò |
| --- | --- |
| **Rust core** (`core/`, `tui/`) | chủ lực: dò, lập kế hoạch, an toàn, cài |
| **Python** (`python/`) | kiểm tra, gợi ý, chẩn đoán, test |
| **Java** (`java/`) | catalog gói, profile desktop, plugin, JSON-RPC |

Quy tắc giữa chúng: **chỉ Rust core được thay đổi hệ thống.**

Xem [docs/vi/architecture.md](docs/vi/architecture.md) và
[docs/vi/ipc-protocol.md](docs/vi/ipc-protocol.md).

## Quy trình cài

```
Chào → Ngôn ngữ → Bàn phím → Múi giờ → Lưu trữ → Chế độ ổ đĩa →
Bố cục → Hệ thống tệp → Tài khoản → Bootloader → Desktop → Dịch vụ →
Gói → Xem lại (gõ cụm xác nhận) → Tiến trình → Kết quả
```

Chỉ bàn phím: `↑↓` di chuyển, `Enter` chọn, `Space` bật/tắt, `Tab` ô kế,
`Esc` quay lại, `F1` trợ giúp, `F5` quét lại, `F10` xem lại, `Ctrl-C` thoát.

## Kiểm thử

```bash
./build.sh test                                # tất cả (Rust + Python + Java)
cd python && python3 -m unittest discover -s tests -v
cd java && ./build.sh
python scripts/check_lang_table.py              # bảng tài liệu trỏ đúng ngôn ngữ
python scripts/check_pkgbuild.py                # PKGBUILD khớp thẻ mới nhất
cargo clippy --workspace --all-targets -- -D warnings
```

Bản cài thật được kiểm trong VirtualBox — xem
[docs/vi/testing.md](docs/vi/testing.md).

## I USE ARCH BTW

<p align="center">
  <img src="assets/arch.webp" alt="Arch Linux" width="110">
</p>

Cài Arch là một lựa chọn, không phải một công việc nặng nhọc. camareLOL tồn
tại để lựa chọn đó vẫn thuộc về bạn: bạn đọc được toàn bộ kế hoạch trước khi
bất cứ thứ gì được ghi, và bạn có thể dừng lại bất cứ lúc nào trước khi gõ
cụm xác nhận.

## Giấy phép

GPL-3.0-or-later. Xem phần đầu mỗi tệp.

## Cảnh báo

Cài hệ điều hành và phân vùng ổ đĩa **có thể phá huỷ dữ liệu**. camareLOL hỏi
trước mỗi bước không thể hoàn tác, định danh lại ổ đĩa trước khi ghi, và
không bao giờ tuyên bố thành công điều nó chưa kiểm chứng. Dù sao: hãy sao
lưu mọi thứ bạn quan tâm trước khi cài bất cứ thứ gì.