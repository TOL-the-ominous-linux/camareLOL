<p align="center">
  <img src="assets/camareLOL.png" alt="camareLOL" width="140">
</p>

# camareLOL

[![m0QpBjbzyd](https://yourimageshare.com/ib/m0QpBjbzyd.gif)](https://yourimageshare.com/ib/m0QpBjbzyd)

> Trình cài đặt Arch Linux chạy trong terminal, ưu tiên an toàn — có linh hồn Việt.

[Tiếng Anh](README.md) · **Tiếng Việt** · [中文](docs/zh/index.md)

> ## ⚠ BETA — 1.0.1 beta
>
> camareLOL **1.0.1** là bản beta hiện tại; **1.0.0** là bản beta đầu tiên.
> Đợt rà soát roadmap đã xong: mỗi mục hoặc đã có bằng chứng chạy thật, hoặc
> được ghi rõ là **một phần**, hoặc bị hoãn kèm lý do. Điều đó không có nghĩa
> là "đã hoàn hảo" — ba mục desktop vẫn còn một phần, dual-boot với Windows
> thật vẫn bị hoãn, và chương trình này xoá ổ đĩa.
>
> Các bản trước đã chứng minh vì sao phải cần bằng chứng: ba tên gói Arch viết
> sai đã qua **toàn bộ** unit test và chỉ chết trong VirtualBox —
> `greetd-tuigreeter`, `p7zip`, và một gói tên `systemd-boot` không tồn tại.
> Giờ đã có test canh đúng những lỗi đó, và harness VM kiểm tra đĩa đã cài
> chứ không kiểm tra menu.
>
> **Vì vậy:**
>
> 1. Chỉ chạy trên **ổ trống** — ổ đang có Windows hay dữ liệu thì đừng.
> 2. Chạy từ **Arch live ISO**, không phải trên máy bạn đang dùng.
> 3. Sao lưu mọi thứ trước khi cài bất cứ thứ gì.
>
> `1.0.1` là beta, không phải lời hứa ổn định. Thẻ npm `latest` vẫn là
> **0.1.22**; bản beta nằm ở dist-tag `beta` và là GitHub pre-release. Hãy
> cài có chủ ý — xem [Cài 1.0.1 beta](#cài-101-beta).
>
> Ghi chú phát hành đầy đủ, gồm bằng chứng và giới hạn đã biết:
> [docs/releases/1.0.1.md](docs/releases/1.0.1.md). Ghi chú của bản beta đầu
> tiên vẫn ở [docs/releases/1.0.0.md](docs/releases/1.0.0.md).

camareLOL cài Arch Linux từ terminal mà không bắt bạn gõ hàng chục lệnh
shell. Nó dò phần cứng, đề xuất cách phân vùng an toàn, cho bạn xem chính
xác nó sắp làm gì, và chỉ ghi vào ổ đĩa sau khi bạn xác nhận.

**Nó không phải lớp bọc shell.** Trình cài đặt có quy trình riêng: phát hiện →
lập kế hoạch → xem lại → xác nhận → thực thi → kiểm tra, với một cổng an
toàn đứng trước mọi thao tác phá huỷ.

| | |
|---|---|
| Phát hành | <https://github.com/TOL-the-ominous-linux/camareLOL/releases> |
| Thảo luận | <https://github.com/TOL-the-ominous-linux/camareLOL/discussions> |
| Vấn đề | <https://github.com/TOL-the-ominous-linux/camareLOL/issues> |

## Tài liệu

Viết bằng ba ngôn ngữ. Mỗi trang có bảng trỏ sang hai ngôn ngữ còn lại.

| Chủ đề | Tiếng Việt | English | 中文 |
|---|---|---|---|
| Tổng quan | [index](docs/vi/index.md) | [EN](docs/en/index.md) | [ZH](docs/zh/index.md) |
| Ghi chú 1.0.1 | [releases/1.0.1](docs/releases/1.0.1.md) | — | — |
| Ghi chú 1.0.0 | [releases/1.0.0](docs/releases/1.0.0.md) | — | — |
| Ba thành phần | [architecture](docs/vi/architecture.md) | [EN](docs/en/architecture.md) | [ZH](docs/zh/architecture.md) |
| Giao thức IPC | [ipc-protocol](docs/vi/ipc-protocol.md) | [EN](docs/en/ipc-protocol.md) | [ZH](docs/zh/ipc-protocol.md) |
| Được kiểm tra gì | [testing](docs/vi/testing.md) | [EN](docs/en/testing.md) | [ZH](docs/zh/testing.md) |
| Tự chạy thử | [vm-testing](docs/vi/vm-testing.md) | [EN](docs/en/vm-testing.md) | [ZH](docs/zh/vm-testing.md) |
| Cập nhật | [update](docs/vi/update.md) | [EN](docs/en/update.md) | [ZH](docs/zh/update.md) |
| Chưa làm gì | [roadmap](docs/vi/roadmap.md) | [EN](docs/en/roadmap.md) | [ZH](docs/zh/roadmap.md) |

Bắt đầu với [tổng quan](docs/vi/index.md).

## Thay đổi trong 1.0.1

**Sửa lỗi dò đĩa và self-test.** `lsblk` gọi zram, ram, nbd, md và loop là
`disk`; camareLOL đã liệt kê chúng trên màn hình Storage. Trên máy có zram,
`--self-test` chọn zram làm ổ trống, trượt kiểm tra dung lượng và báo FAIL.
Một zram đủ lớn để qua ngưỡng tối thiểu sẽ trở thành đích cài vào bộ nhớ
volatile.

1.0.1 lọc các thiết bị khối ảo đó ngay khi dò, và `--self-test` bỏ qua phần
cài mô phỏng khi không có ổ trống thay vì chạy nó trên hệ thống đang dùng.
Không có thay đổi nào ở lập kế hoạch, phân vùng, `pacstrap`, bootloader,
desktop, safety gate, IPC hay schema cấu hình.

Ghi chú đầy đủ: [docs/releases/1.0.1.md](docs/releases/1.0.1.md).

## Cài 1.0.1 beta

Bản beta là tuỳ chọn. Lệnh một dòng mặc định vẫn lấy bản **ổn định** mới nhất
(0.1.22). Muốn cài beta, ghim tag hoặc chọn kênh beta:

```console
$ curl -fsSL https://raw.githubusercontent.com/TOL-the-ominous-linux/camareLOL/main/scripts/camarelol.sh \
    | CAMARELOL_VERSION=v1.0.1 bash
```

```console
$ curl -fsSL https://raw.githubusercontent.com/TOL-the-ominous-linux/camareLOL/main/scripts/camarelol.sh \
    | CAMARELOL_CHANNEL=beta bash
```

Qua npm:

```console
$ npm install -g @wenker-ai/camarelo@beta
```

## Cài đặt

### Một dòng (cầu nối)

Cách nhanh nhất, và không thể cài nhầm thứ:

```console
$ curl -fsSL https://raw.githubusercontent.com/TOL-the-ominous-linux/camareLOL/main/scripts/camarelol.sh | bash
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

Tải binary từ [Releases](https://github.com/TOL-the-ominous-linux/camareLOL/releases)
rồi chạy từ Arch live ISO:

```console
$ chmod +x camarelol
$ ./camarelol --self-test      # kiểm không phá huỷ; an toàn trên máy thật
$ sudo ./camarelol             # mở TUI
```

### Qua npm

```console
$ sudo pacman -S --needed nodejs npm curl
$ npm install -g @wenker-ai/camarelo          # latest ổn định: 0.1.22
$ npm install -g @wenker-ai/camarelo@beta     # beta 1.0.1
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
- Phát hiện Windows và **không chạm vào nó**. Dual-boot kiểu giữ mọi phân vùng
  đã chạy thật: phân vùng cũ vẫn `format=false`, ESP được dùng lại, phân vùng
  mới được thêm vào khe trống; đã xác nhận cài và boot lần đầu trên máy thật
  ngày 24/10/2026.
- Cài Arch tối thiểu: pacstrap, fstab, bootloader (GRUB/systemd-boot), locale,
  keymap, timezone, hostname, user, sudo, service, initramfs.
- Cổng an toàn: khóa sẵn. Cần `--allow-destructive` + cụm xác nhận.
- Cấu hình lưu/đọc, có số phiên bản, **không chứa mật khẩu**.
- Ghi nhật ký mọi pha, có che bí mật.

## Cố ý chưa làm gì

- Xoá đĩa chỉ vì bạn chọn nó trong menu.
- Cài gói AUR khi chưa có đồng ý rõ ràng và một bản build bởi người dùng thường.
- Thu nhỏ NTFS — tiêu chí chấp nhận đã viết, tính năng để dành.
- Boot Windows thật từ entry menu do camareLOL sinh ra, hoặc thu nhỏ Windows từ
  bên trong Windows. Phần giữ nguyên phân vùng và entry menu đã được chứng
  minh trong VM; phần Windows thật vẫn bị hoãn.
- Gợi ý driver GPU theo model. Model đã được đọc và hiển thị; phần chọn driver
  theo model vẫn còn một phần.
- Chia phân vùng thủ công, LUKS, và bố cục Btrfs subvolume — chưa có mã.

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

## Thuộc The Ominous Linux

camareLOL là dự án đầu tiên của **The Ominous Linux (TOL)** — tên gọi cho những
công cụ Linux nói thẳng rủi ro và đưa ra bằng chứng. TOL cũng là nơi đặt dòng
**LOL kernel** đang được lên kế hoạch.

- [TOL là gì](tol/README.md) — trang chủ:
  <https://github.com/TOL-the-ominous-linux/.github>
- [Thương hiệu và cách dùng logo](tol/BRAND.md) — biểu tượng lấy cảm hứng từ
  ban nhạc TOOL và luôn đi kèm dòng ghi công đó
- [LOL kernel](tol/LOL-KERNEL.md) — đang lên kế hoạch, chưa phát hành

## I USE ARCH BTW

<p align="center">
  <img src="assets/arch.png" alt="Arch Linux" width="110">
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