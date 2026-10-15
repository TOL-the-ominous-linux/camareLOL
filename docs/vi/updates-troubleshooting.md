<!-- Dịch từ docs/en/updates-troubleshooting.md. Sửa bản gốc rồi dịch lại; đừng sửa bản này. -->
# Cập nhật: khi nào bản cập nhật bị từ chối

[← Chỉ mục tài liệu](index.md)

Mọi trường hợp camareLOL có thể từ chối, nguyên nhân, và cách xử lý.

## Đọc một thông báo lỗi

```console
error: [update_download_failed] could not download https://…
  (curl: (22) The requested URL returned error: 404)
  → try the next mirror, or pass --update-url to name one yourself
```

Ba phần: một mã ổn định đọc được bằng máy, một câu nói điều gì đã xảy ra, và một
mũi tên chỉ hành động tiếp theo. Các mã bên dưới lấy từ `core/src/error.rs`;
chúng ổn định qua các bản phát hành, nên một script có thể khớp theo chúng.

## Chỉ mục và mạng

### `update_download_failed`

Mọi mirror đều không truy cập được, hoặc một proxy trả về trang lỗi.

```console
$ curl -sI https://camarelol.netlify.app/latest.json
$ camarelol --check-update --update-url https://example.org/latest.json
```

Một mirror trả về **200 kèm trang HTML** không phải là thành công. Một số máy
chủ tĩnh lùi về `index.html` cho các đường dẫn lạ, nên 200 ở đó nghĩa là "không
tìm thấy" mà mặc mã thành công. Hãy kiểm tra kiểu nội dung, không chỉ mã trạng
thái.

### `update_manifest_invalid`

Chỉ mục không có hình dạng mà client có thể hành động: `schema` sai, thiếu một
trường, URL tải về không phải HTTPS, hoặc `sha256` không phải 64 ký tự hex.

Đây là sự từ chối chứ không phải một phỏng đoán. Một chỉ mục mà camareLOL không
thể hiểu hết thì không nên được cài từ đó.

### `update_url_refused`

```console
error: [update_url_refused] this build was packaged without an update channel
  → updates come from the package manager that built it, or pass --update-url
```

Tệp nhị phân được build với `CAMARELOL_NO_UPDATE=1`. Hành vi này là đúng theo
thiết kế — xem [tự đóng gói bản của riêng bạn](updates-packaging.md).

URL HTTP thuần bị từ chối vì cùng lý do đó, trừ khi nó là loopback: mọi thứ
không phải `127.0.0.1` đều phải là `https://`.

## Lòng tin

### `update_signature_missing`

Chỉ mục đến mà không có chữ ký. Mirror đang phục vụ `latest.json` mà không có
`latest.json.sig`.

Hãy phục vụ cả hai. Nếu bạn kiểm soát mirror và thật sự không thể ký, hãy đọc
[cập nhật: ký](updates-signing.md) trước — ghi đè nghĩa là chấp nhận mọi nội
dung tải về từ địa chỉ đó.

### `update_signature_invalid`

Chữ ký không xác minh được với khoá được ghim.

Thường là do tải lên bị cắt cụt, sai tệp, hoặc chữ ký được tạo bằng một khoá
khác:

```console
$ gpg --verify latest.json.sig latest.json
```

Khoá đã ký phải là khoá trong `keys/camarelol.pub`. Hãy xác minh trước khi tải
lại bất cứ thứ gì.

### `update_expired`

Chỉ mục đã vượt qua `expires` của nó. Nó được ký đúng và chỉ đơn giản là quá
cũ — xem [vì sao chỉ mục cũ bị từ chối](updates-rollback.md).

Hoặc mirror chưa làm mới nó, hoặc máy này đã ngoại tuyến quá một tuần.
`--allow-expired-index` sẽ tiếp tục nếu bạn biết lý do.

## Nội dung tải về

### `update_checksum_mismatch`

Số byte đã tải về không khớp với `sha256` trong chỉ mục.

Do tải về bị cắt cụt, một proxy ghi lại phần thân, hoặc tệp đính kèm của bản phát
hành bị thay thế sau khi chỉ mục đã được ký.

**Đừng** thử lại một cách mù quáng. Hãy so sánh thứ bạn có với thứ trang phát
hành liệt kê.

### `update_candidate_failed`

Tệp nhị phân mới đã tải về và xác minh được nhưng không chạy được `--version`.

Thiếu thư viện, hoặc sai kiến trúc. Tệp nhị phân cũ được giữ nguyên tại chỗ —
chính phép kiểm tra này là lý do việc thay thế chỉ xảy ra sau khi bản sao mới
đã chứng minh nó khởi động được.

## Công tắc chuyển

### `update_not_privileged`

Người dùng hiện tại không có quyền ghi vào tệp nhị phân.

```console
$ sudo camarelol --update
```

### Đang có một quá trình cài đặt chạy

`an installation is running`

Thay tệp nhị phân giữa lúc cài là cách nhanh nhất để để lại một máy cài đúng
nửa, nên trình cập nhật từ chối. Điều này giữ nguyên kể cả khi chỉ mục được ký
đúng — một chữ ký hợp lệ không được biến thành lối đi vòng qua quy tắc an toàn.

### Bản cập nhật sẽ không thay thế tệp nhị phân đang chạy

Nội dung tải về nằm ở một chỗ khác với tệp thực thi đang chạy. Hãy truyền
`--update-url` và chép tệp một cách tường minh, hoặc cài qua trình quản lý gói.

### `update_rollback_failed`

`--rollback-update` không tìm thấy tệp nhị phân `.prev` để khôi phục. Nó chỉ
tồn tại nếu một bản cập nhật đã cài thành công ngay từ đầu.

## Những thứ không phải là lỗi

| thông báo | ý nghĩa |
|---|---|
| `up to date` | chỉ mục khớp hoặc cũ hơn bản của bạn |
| `version 0.1.11 is available` | được báo cáo, không tải gì |
| mã thoát `10` | có bản mới hơn và không làm gì cả |

Mã thoát 10 tồn tại để một script có thể nhắc nhở mà không bị coi là thất bại:

```console
camarelol --check-update || [ $? = 10 ] || exit 1
```

## Khi không có cách nào ăn

Kiểm tra từng bước bằng tay:

```console
$ curl -sS https://camarelol.netlify.app/latest.json | head -3
$ curl -sSI https://camarelol.netlify.app/latest.json.sig | head -1
$ camarelol --check-update --update-url https://… --update-key ./keys/camarelol.pub
```

Nếu một mirror có vấn đề, hãy nói ra — một mirror chết phải được người vận
hành nó phát hiện, chứ không phải người lần sau cố cập nhật.

- <https://camarelol.netlify.app/status.html> — trạng thái mirror trực tiếp
- <https://github.com/tamkogay9876-code/camareLOL/issues> — báo cáo

## Tiếp theo

- [Cập nhật: ký](updates-signing.md)
- [Vì sao chỉ mục cũ bị từ chối](updates-rollback.md)
- [Tự đóng gói bản của riêng bạn](updates-packaging.md)
