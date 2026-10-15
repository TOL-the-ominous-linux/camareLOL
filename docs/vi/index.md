<!-- Dịch từ docs/en/index.md. Sửa bản gốc rồi dịch lại; đừng sửa bản này. -->
# camareLOL

> Trình cài đặt TUI cho Arch Linux, ưu tiên an toàn, hỗ trợ ba ngôn ngữ.

camareLOL cài Arch Linux từ terminal. Nó phát hiện phần cứng của bạn, đề xuất
một cách phân vùng, cho bạn xem chính xác nó sắp làm gì, và chỉ ghi vào ổ đĩa
sau khi bạn gõ xong cụm từ xác nhận.

**Đây không phải là một trình bao bọc shell.** Trình cài đặt có quy trình riêng —
khám phá → lập kế hoạch → xem lại → xác nhận → thực thi → kiểm tra — với một
cổng an toàn đứng trước mọi thao tác phá huỷ.

| | |
|---|---|
| Trang web | **<https://tamkogay9876-code.github.io/camareLOL-mirror>** |
| Bản phát hành | <https://github.com/tamkogay9876-code/camareLOL/releases> |
| Thảo luận | <https://github.com/tamkogay9876-code/camareLOL/discussions> |
| Vấn đề | <https://github.com/tamkogay9876-code/camareLOL/issues> |

## Ngôn ngữ

| Ngôn ngữ | Tài liệu |
|---|---|
| English | <https://tamkogay9876-code.github.io/camareLOL-mirror/en/docs/> |
| Tiếng Việt | <https://tamkogay9876-code.github.io/camareLOL-mirror/vi/docs/> |
| 中文 | <https://tamkogay9876-code.github.io/camareLOL-mirror/zh/docs/> |

Bản thân trình cài đặt nói tiếng Việt và tiếng Anh (`--lang vi|en`).

## Cài đặt

Chạy nó từ ISO sống của Arch — đó là nơi một trình cài đặt thuộc về:

```console
$ curl -sSLO https://github.com/tamkogay9876-code/camareLOL/releases/latest/download/camarelol
$ chmod +x camarelol
$ ./camarelol --self-test      # kiểm không phá huỷ; an toàn trên máy thật
$ sudo ./camarelol             # mở TUI
```

`--self-test` không bao giờ ghi vào ổ đĩa. Hãy chạy nó trước.

Luôn cập nhật — trình cập nhật chỉ báo cáo và chỉ cài khi được yêu cầu:

```console
$ camarelol --check-update     # chỉ báo, không tải gì, mã thoát 10 nếu có bản mới
$ camarelol --update           # tải, kiểm chữ ký và SHA-256, cài
$ camarelol --rollback-update  # đưa binary cũ trở lại
```

Nó không bao giờ tự thay thế chính nó, không bao giờ làm việc đó khi một quá
trình cài đặt đang chạy, và từ chối mọi bản tải về mà tổng kiểm hoặc chữ ký
không khớp.

## Tài liệu

| Chủ đề | English | Tiếng Việt | 中文 |
|---|---|---|---|
| Cách ký bản cập nhật | [updates-signing](updates-signing.md) | [VI](updates-signing.md) | [ZH](https://tamkogay9876-code.github.io/camareLOL-mirror/zh/docs/updates-signing.html) |
| Vì sao chỉ mục cũ bị từ chối | [updates-rollback](updates-rollback.md) | [VI](updates-rollback.md) | [ZH](https://tamkogay9876-code.github.io/camareLOL-mirror/zh/docs/updates-rollback.html) |
| Tự đóng gói bản của riêng bạn | [updates-packaging](updates-packaging.md) | [VI](updates-packaging.md) | [ZH](https://tamkogay9876-code.github.io/camareLOL-mirror/zh/docs/updates-packaging.html) |
| Khi nào bản cập nhật bị từ chối | [updates-troubleshooting](updates-troubleshooting.md) | [VI](updates-troubleshooting.md) | [ZH](https://tamkogay9876-code.github.io/camareLOL-mirror/zh/docs/updates-troubleshooting.html) |
| Ba thành phần | [architecture](architecture.md) | [VI](architecture.md) | [ZH](https://tamkogay9876-code.github.io/camareLOL-mirror/zh/docs/architecture.html) |
| Giao thức IPC | [ipc-protocol](ipc-protocol.md) | [VI](ipc-protocol.md) | [ZH](https://tamkogay9876-code.github.io/camareLOL-mirror/zh/docs/ipc-protocol.html) |
| Những gì được kiểm thử | [testing](testing.md) | [VI](testing.md) | [ZH](https://tamkogay9876-code.github.io/camareLOL-mirror/zh/docs/testing.html) |
| Tự chạy thử | [vm-testing](vm-testing.md) | [VI](vm-testing.md) | [ZH](https://tamkogay9876-code.github.io/camareLOL-mirror/zh/docs/vm-testing.html) |
| Những gì chưa làm | [roadmap](roadmap.md) | [VI](roadmap.md) | [ZH](https://tamkogay9876-code.github.io/camareLOL-mirror/zh/docs/roadmap.html) |

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

- Định dạng ổ đĩa chỉ vì bạn chọn nó trong một menu.
- Cài gói AUR khi chưa có đồng ý rõ ràng và một bản build bởi người dùng thường.
- Thu nhỏ phân vùng NTFS — tiêu chí nghiệm thu đã được ghi, nhưng tính năng bị
  hoãn lại.
- Dùng lại một ESP sẵn có cho dual-boot. Quy tắc phát hiện và giữ nguyên đã
  được hiện thực; giao diện bố trí phân vùng sẽ đến ở Stage 3.

## Luồng trình cài đặt

```
Welcome → Language → Keyboard → Timezone → Storage → Disk mode →
Layout → Filesystem → Accounts → Bootloader → Desktop → Services →
Packages → Review (type the confirmation phrase) → Progress → Result
```

Chỉ dùng bàn phím. `↑↓` di chuyển, `Enter` chọn, `Space` bật/tắt, `Tab` sang
ô tiếp theo, `Esc` quay lại, `F1` trợ giúp, `F5` quét lại, `F10` xem lại,
`Ctrl-C` thoát, `u` kiểm tra cập nhật.

## Cảnh báo

Cài một hệ điều hành và phân vùng ổ đĩa **có thể phá huỷ dữ liệu**.
camareLOL hỏi trước mỗi bước không thể hoàn tác, xác định lại ổ đĩa trước khi
ghi, và không bao giờ tuyên bố thành công cho thứ nó chưa kiểm chứng.

Vẫn nên nhớ: sao lưu mọi thứ bạn quan tâm trước khi cài bất cứ thứ gì, và hãy
chạy nó từ ISO sống thay vì chạy trên chính chiếc máy bạn đang cố sửa.
