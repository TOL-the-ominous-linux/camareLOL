<!-- Dịch từ docs/en/index.md. Sửa bản gốc rồi dịch lại; đừng sửa bản này. -->
# camareLOL

> Trình cài đặt TUI cho Arch Linux, ưu tiên an toàn, hỗ trợ ba ngôn ngữ.

**Bản hiện tại:** `1.0.1` — beta (GitHub pre-release). Thẻ npm
`latest` vẫn là `0.1.22`; bản beta là `@wenker-ai/camarelo@beta`.
Đọc [ghi chú phát hành 1.0.1](../releases/1.0.1.md). Ghi chú của bản beta đầu
tiên vẫn ở [1.0.0](../releases/1.0.0.md).

camareLOL cài Arch Linux từ terminal. Nó phát hiện phần cứng của bạn, đề xuất
một cách phân vùng, cho bạn xem chính xác nó sắp làm gì, và chỉ ghi vào ổ đĩa
sau khi bạn gõ xong cụm từ xác nhận.

**Đây không phải là một trình bao bọc shell.** Trình cài đặt có quy trình riêng —
khám phá → lập kế hoạch → xem lại → xác nhận → thực thi → kiểm tra — với một
cổng an toàn đứng trước mọi thao tác phá huỷ.

| | |
|---|---|
| Bản phát hành | <https://github.com/tamkogay9876-code/camareLOL/releases> |
| Thảo luận | <https://github.com/tamkogay9876-code/camareLOL/discussions> |
| Vấn đề | <https://github.com/tamkogay9876-code/camareLOL/issues> |

## Ngôn ngữ

Bản thân trình cài đặt nói tiếng Việt, tiếng Anh và tiếng Trung giản thể (`--lang vi|en|zh`).

## Cài đặt

Chạy nó từ ISO sống của Arch — đó là nơi một trình cài đặt thuộc về. Lệnh một
dòng mặc định lấy bản **ổn định** mới nhất (0.1.22); beta 1.0.1 là tuỳ chọn:

```console
$ curl -fsSL https://raw.githubusercontent.com/tamkogay9876-code/camareLOL/main/scripts/camarelol.sh \
    | CAMARELOL_VERSION=v1.0.1 bash
$ camarelol --self-test      # kiểm không phá huỷ; an toàn trên máy thật
$ sudo camarelol             # mở TUI
```

Dùng `CAMARELOL_CHANNEL=beta` thay cho `CAMARELOL_VERSION` để theo bản
pre-release mới nhất. Bỏ cả hai để dùng kênh ổn định.

`--self-test` không bao giờ ghi vào ổ đĩa. Hãy chạy nó trước.

Nếu bạn muốn tự tải binary, nhớ giữ `-f` và `-L`:

```console
$ curl -fLO https://github.com/tamkogay9876-code/camareLOL/releases/download/v1.0.1/camarelol
$ curl -fLO https://github.com/tamkogay9876-code/camareLOL/releases/download/v1.0.1/SHA256SUMS
$ sha256sum -c SHA256SUMS
```

Bỏ `-f` thì một proxy trả trang 404 vẫn được lưu thành tệp tên `camarelol` và
curl vẫn trả mã 0; `chmod +x` cũng thành công trên một tệp văn bản, rồi lần
chạy đầu báo `line 1: Not: command not found`.

camareLOL không tự cập nhật. Không có trình cập nhật, không có mã chỉ mục,
và trong binary không có đường nào thay chính nó.

Đó là quyết định, không phải phần chưa làm. Chương trình này xoá ổ đĩa; một
thành phần việc của nó là thay thế chính nó là con đường ngắn nhất từ máy
chủ bị chiếm tới dữ liệu bị xoá. Không có gói `pacman` và cũng không có gói
AUR. Chạy lại dòng một dòng ở trên; nó tự lấy bản **ổn định** mới nhất rồi
kiểm tra:

```console
$ curl -fsSL https://raw.githubusercontent.com/tamkogay9876-code/camareLOL/main/scripts/camarelol.sh | bash
```

Hoặc tải bản bạn muốn từ
<https://github.com/tamkogay9876-code/camareLOL/releases> và thay binary
bằng tay.

## Tài liệu

| Chủ đề | English | Tiếng Việt | 中文 |
|---|---|---|---|
| Ba thành phần | [EN](../en/architecture.md) | [VI](architecture.md) | [ZH](../zh/architecture.md) |
| Giao thức IPC | [EN](../en/ipc-protocol.md) | [VI](ipc-protocol.md) | [ZH](../zh/ipc-protocol.md) |
| Những gì được kiểm thử | [EN](../en/testing.md) | [VI](testing.md) | [ZH](../zh/testing.md) |
| Tự chạy thử | [EN](../en/vm-testing.md) | [VI](vm-testing.md) | [ZH](../zh/vm-testing.md) |
| Cập nhật | [EN](../en/update.md) | [VI](update.md) | [ZH](../zh/update.md) |
| Những gì chưa làm | [EN](../en/roadmap.md) | [VI](roadmap.md) | [ZH](../zh/roadmap.md) |

## Hiện nó làm được gì

- Phát hiện ổ đĩa, phân vùng, hệ thống tệp, phân vùng EFI, Windows, phân vùng
  khôi phục và phân vùng dành riêng cho Microsoft, cùng các điểm gắn của hệ
  thống sống.
- Lập kế hoạch cách phân vùng GPT và MBR: ESP, BIOS boot, `/`, `/home`, swap,
  với ext4, btrfs hoặc xfs, được kiểm tra theo chế độ firmware.
- Phát hiện Windows và từ chối đụng vào nó. Cách phân vùng dual-boot mới chỉ
  được lập kế hoạch, không bao giờ thực thi mù quáng.
- Cài một hệ thống Arch tối giản: pacstrap, fstab, trình nạp khởi động, locale,
  keymap, múi giờ, hostname, người dùng, chính sách sudo, service, initramfs.
- Cổng an toàn: khóa mặc định. `--allow-destructive` cộng với một cụm từ xác
  nhận phải gõ tay (`ERASE`, `ERASE AND WIPE`) để mở khóa; `--execute` là bắt
  buộc trước khi bất cứ thứ gì được ghi.
- Hồ sơ cấu hình: lưu và nạp, có phiên bản schema, **không bao giờ chứa mật
  khẩu**.
- Ghi nhật ký từng giai đoạn kèm che giấu thông tin bí mật.

## Những gì nó cố ý chưa làm

- Chạy trên bất cứ thứ gì ngoài Arch Linux với systemd. Hệ điều hành và hệ
  thống init được đọc từ `/etc/os-release` và `/proc/1/comm`; bất kỳ thứ gì
  khác đều bị từ chối ngay ở bước lập kế hoạch, trước khi ghi bất kỳ byte
  nào. **Parabola và Hyperbola là bản phân phối GNU/Linux-libre, không phải
  Arch** — chúng dùng `pacman` nhưng bỏ hoặc thay thế các gói không tự do, nên
  một danh sách gói đúng trên Arch có thể không tồn tại hoặc bị đổi tên ở đó.
  Artix, Nyarch và các bản phân phối `ID_LIKE=arch` như EndeavourOS cũng bị
  từ chối vì cùng lý do: chúng là những hệ chưa được kiểm chứng, không phải
  Arch.
- Định dạng ổ đĩa chỉ vì bạn chọn nó trong một menu.
- Cài gói AUR khi chưa có đồng ý rõ ràng và một bản build bởi người dùng thường.
- Thu nhỏ phân vùng NTFS — tiêu chí nghiệm thu đã được ghi, nhưng tính năng bị
  hoãn lại.
- Boot Windows thật từ entry menu do camareLOL sinh ra, hoặc thu nhỏ Windows từ
  bên trong Windows. Planner giữ mọi phân vùng, đường ghi chỉ thêm phân vùng,
  và entry menu Windows đã được chứng minh trong VM; phần Windows thật bị hoãn.
- Gợi ý driver GPU theo model. `GpuInfo.model` đã được đọc và hiển thị; phần
  chọn driver theo model vẫn còn một phần.

## Luồng trình cài đặt

```
Welcome → Language → Keyboard → Timezone → Storage → Disk mode →
Layout → Filesystem → Accounts → Bootloader → Desktop → Services →
Packages → Review (type the confirmation phrase) → Progress → Result
```

Chỉ dùng bàn phím. `↑↓` di chuyển, `Enter` chọn, `Space` bật/tắt, `Tab` sang
ô tiếp theo, `Esc` quay lại, `F1` trợ giúp, `F5` quét lại, `F10` xem lại,
`Ctrl-C` thoát.

## Cảnh báo

Cài một hệ điều hành và phân vùng ổ đĩa **có thể phá huỷ dữ liệu**.
camareLOL hỏi trước mỗi bước không thể hoàn tác, xác định lại ổ đĩa trước khi
ghi, và không bao giờ tuyên bố thành công cho thứ nó chưa kiểm chứng.

Vẫn nên nhớ: sao lưu mọi thứ bạn quan tâm trước khi cài bất cứ thứ gì, và hãy
chạy nó từ ISO sống thay vì chạy trên chính chiếc máy bạn đang cố sửa.
