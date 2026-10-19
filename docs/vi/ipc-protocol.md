<!-- Dịch từ docs/en/ipc-protocol.md. Sửa bản gốc rồi dịch lại; đừng sửa bản này. -->
# Giao thức IPC, phiên bản 1

[← Chỉ mục tài liệu](index.md)

JSON phân tách theo dòng mới: mỗi yêu cầu một dòng, mỗi câu trả lời một dòng.

Chỉ lõi Rust được phép thực hiện thao tác thay đổi hệ thống. Python và Java nói
chuyện với nó — hoặc với máy chủ riêng của danh mục Java — qua giao thức này.

## Định dạng trên dây

```jsonc
// request
{"id": 1, "protocol_version": 1, "method": "scan_devices", "params": {}, "timeout_ms": 120000}

// response
{"id": 1, "protocol_version": 1, "status": "ok", "result": {}, "duration_ms": 3}
```

| trường | kiểu | ý nghĩa |
| --- | --- | --- |
| `id` | số hoặc chuỗi | được trả lại nguyên vẹn; phải khác 0 |
| `protocol_version` | số | phải là `1` |
| `method` | chuỗi | xem bên dưới |
| `params` | đối tượng | đối số của method |
| `timeout_ms` | số, tuỳ chọn | thời gian chờ theo từng yêu cầu; mặc định 120 000 |
| `cancel` | số, tuỳ chọn | id của một yêu cầu trước đó cần huỷ |

## Các method

| method | phá huỷ | mô tả |
| --- | --- | --- |
| `hello` | không | khám phá giao thức; kết quả có `host`, ví dụ `Arch Linux / systemd` |
| `scan_devices` | không | khảo sát lưu trữ đầy đủ, dưới dạng mảng JSON |
| `create_plan` | không | dựng một kế hoạch phân vùng cho `params.disk` |
| `validate_plan` | không | kiểm tra lại một kế hoạch đã có |
| `apply_plan` | **có** | ghi bảng phân vùng; cần một cổng an toàn đã mở khóa |
| `format_partitions` | **có** | tạo hệ thống tệp; cần một cổng an toàn đã mở khóa |
| `diagnostics` | không | báo cáo chẩn đoán |
| `get_config` / `set_config` | không | đọc và ghi cấu hình, được kiểm tra |
| `recommend_packages` | không | uỷ quyền cho Python; chỉ mang tính thông tin |

## Lỗi

```jsonc
{"id": 2, "protocol_version": 1, "status": "error",
 "error": {"code": "safety_gate_closed",
           "message": "refusing to write partition table: destructive mode is not enabled",
           "remediation": "restart camareLOL with --allow-destructive"},
 "duration_ms": 1}
```

Một lỗi chính là một câu trả lời. Không có kênh lỗi riêng, và một phía gọi chỉ
đọc `result` mà bỏ qua `status` sẽ đọc một thất bại thành thành công.

### Mã lỗi ổn định

| nhóm | mã |
| --- | --- |
| tools | `missing_tool`, `spawn_failed`, `command_failed` |
| parsing | `parse_failed`, `unsupported_schema`, `invalid_config`, `invalid_plan` |
| storage | `no_target_disk`, `device_gone`, `device_identity_changed`, `device_busy` |
| boot and accounts | `invalid_boot_config`, `invalid_account` |
| network | `network_unreachable` |
| safety | `safety_gate_closed`, `confirmation_required`, `unsupported_host`, `aur_build_refused` |
| updates | `update_download_failed`, `update_manifest_invalid`, `update_signature_missing`, `update_signature_invalid`, `update_expired`, `update_checksum_mismatch`, `update_candidate_failed`, `update_not_privileged`, `update_url_refused`, `update_rollback_failed` |
| other | `not_supported`, `internal` |

## Quy tắc

1. Lõi xác minh mọi yêu cầu, bất kể ai gửi. Việc phía gọi thuộc về dự án này
   không phải là lý do để nới lỏng một phép kiểm tra.
2. Một method phá huỷ bị từ chối trừ khi cổng an toàn đã mở khóa **và** cụm từ
   xác nhận cho mức rủi ro của thao tác đó đã được ghi nhận.
3. Một method phá huỷ cũng bị từ chối với `unsupported_host` khi bản phân phối
   hoặc hệ thống init không phải Arch + systemd. Phép kiểm tra chạy *trước*
   cổng an toàn, để người dùng biết lý do thật thay vì được bảo thêm một cờ mà
   vẫn không làm cho việc cài đặt an toàn hơn. Nó nằm trong `apply_plan` chứ
   không chỉ trong bộ điều phối cài đặt, vì bề mặt IPC tự ghi bảng phân vùng.
4. Một câu trả lời có `status: "error"` không bao giờ được coi là thành công.
5. Thời gian chờ tính theo từng yêu cầu. Một yêu cầu đã huỷ được **trả lời**,
   không bị vứt bỏ — một phía gọi không bao giờ nghe phản hồi thì không phân biệt
   được một thao tác chậm với một thao tác đã treo.
6. `protocol_version` được kiểm tra trước mọi thứ khác. Một phiên bản lạ bị từ
   chối chứ không được phân tích theo phỏng đoán.

## Vì sao phiên bản nằm trong mọi tin nhắn

Vì kiểm tra nó rẻ, còn kiểm tra sau là không thể. Một client kết nối tới một lõi
cũ hơn và hiểu sai một trường trong im lặng sẽ tệ hơn nhiều so với một client từ
chối nói chuyện với nó.

## Ai đang hiện thực nó hôm nay

Giao thức đã được định nghĩa và xác minh, và lõi phơi bày nó nội bộ cho TUI. Nó
**không** được phơi bày như một dịch vụ mạng hay stdin: `camarelol` không có cờ
`--serve`, và không có socket nào đang lắng nghe.

Điều đó là có chủ đích, tạm thời. Một daemon nhận yêu cầu phân vùng qua socket là
một bề mặt tấn công mới trên một công cụ xoá ổ đĩa, và chưa từng cần đến. Thêm
vào là một quyết định phải cân nhắc cẩn thận, không phải một cờ bật lên là xong.

Nếu bạn đang viết một công cụ hôm nay, hãy dùng CLI đã được ghi tài liệu —
`--scan` xuất khảo sát thiết bị dưới dạng JSON, `--scripted` chạy một lần cài
đặt từ một hồ sơ, và cả hai đều ổn định.

## Tiếp theo

- [Kiến trúc](architecture.md)
- [Kiểm thử](testing.md)
