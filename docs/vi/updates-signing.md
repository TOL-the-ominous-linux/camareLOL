<!-- Dịch từ docs/en/updates-signing.md. Sửa bản gốc rồi dịch lại; đừng sửa bản này. -->
# Cập nhật: cách ký chỉ mục

[← Chỉ mục tài liệu](index.md)

Một tổng kiểm chứng minh một tệp đến nơi nguyên vẹn. Nó không chứng minh tệp đó
đến từ đúng người bạn tin — nếu máy chủ đưa cho bạn cả nội dung lẫn hash khớp
với nó, chúng khớp, và sự khớp đó chẳng có ý nghĩa gì.

Vì vậy, chỉ mục cập nhật được ký riêng, và khóa được **biên dịch thẳng vào
trong tệp nhị phân**.

## Chỉ mục

`latest.json` là thứ mà phía client yêu cầu:

```json
{
  "schema": 1,
  "version": "0.1.10",
  "url": "https://github.com/tamkogay9876-code/camareLOL/releases/download/v0.1.10/camarelol",
  "sha256": "fbfe9d8e84fd62b733270932a35113afeb14fa27b6749433d5ddfd0481cc6897"
}
```

| trường | ý nghĩa |
|---|---|
| `schema` | phải là `1`; một client không nhận ra nó sẽ từ chối chứ không đoán |
| `version` | so sánh theo kiểu số, không phải theo chuỗi, nên `0.1.10` mới hơn `0.1.9` |
| `url` | phải là `https://`; URL tải về bằng HTTP thuần bị từ chối |
| `sha256` | 64 ký tự hex, được kiểm tra với số byte đã tải về |

Trường `expires` cũng có thể xuất hiện. Nó được đề cập trong
[vì sao chỉ mục cũ bị từ chối](updates-rollback.md).

## Chữ ký tách rời

`latest.json.sig` là một chữ ký GPG tách rời đè lên **byte thô** của
`latest.json` — được kiểm tra trước khi byte được phân tích, nên một chỉ mục
khéo léo không thể giấu bất cứ thứ gì bằng cách làm sai định dạng.

```
latest.json       the index
latest.json.sig   its signature
```

Cả hai đều được mọi mirror phục vụ. Một mirror phục vụ cái này mà thiếu cái kia
là mirror hỏng, và một client chỉ tìm thấy một trong hai sẽ nói rõ điều đó thay
vì đi tiếp.

## Khoá được ghim

`keys/camarelol.pub` được nhúng vào tệp nhị phân lúc build bằng `include_str!`,
nên nó đi cùng mọi bản sao camareLOL. Không có bước cấu hình nào và cũng không
có keyring nào để bị can thiệp.

Phép kiểm tra là: khoá đã **ký** phải **bằng** khoá được ghim. Không phải "một
khoá mà GPG sẵn sàng tin" — một keyring đã chứa sẵn một khoá do kẻ tấn công thêm
vào sẽ thoả mãn bất kỳ phép kiểm tra nào dễ hơn.

| | |
|---|---|
| Khoá nằm ở đâu | `keys/camarelol.pub`, nhúng lúc build |
| Keyring dùng để xác minh | một thư mục tạm; keyring GPG của bạn không bao giờ bị đụng tới |
| `EXPKEYSIG`, `REVKEYSIG`, `BADSIG` | bị từ chối |
| Chữ ký trên byte khác | bị từ chối |

## Điều gì xảy ra khi không có chữ ký

Chỉ mục không có chữ ký sẽ bị từ chối, và việc từ chối đó không phải là một cảnh
báo:

```console
$ camarelol --update
error: [update_signature_missing] refusing to install an update index that is not signed
  (no public key is configured, so the index cannot be authenticated)
  → publish latest.json.sig and configure a pinned key, or re-run with
    --allow-unsigned-update if you accept the risk
```

`--allow-unsigned-update` tồn tại vì một người đóng gói đứng sau proxy của công ty
có thể không có cách nào để phục vụ một chữ ký. Nó bắt buộc phải gõ tay. Mặc
định là không làm gì cả.

Với một công cụ xoá ổ đĩa, một bản cập nhật không xác thực được thì không đáng
để cài.

## Xoay vòng khoá

Khi một khoá bị xâm nhập, chỉ thay khoá là chưa đủ — các tệp nhị phân đã nằm
trong tay người dùng vẫn mang khoá cũ. Trình tự là:

1. Tạo khoá thay thế và thêm nó vào `keys/camarelol.pub`.
2. Build và ký một bản phát hành bằng khoá mới.
3. Công bố. Khoá cũ ngừng ký mọi thứ, nên mirror không thể phục vụ bất cứ
   thứ gì mới dưới khoá đó.

Việc này đã xảy ra một lần. Khoá bị xâm nhập đã bị xoá, một khoá mới đã ký từ
v0.1.10 trở đi, và việc thu hồi đó chính là lý do một mirror cũ không thể đưa
cho một client hiện tại thứ gì được ký bởi khoá đã bị xâm nhập.

## Dành cho người bảo trì

```console
$ ./scripts/make-signing-key.sh          # once, on your own machine
$ ./scripts/make-release.sh 0.1.10       # builds from the tag, refuses a dirty tree
$ ./scripts/sign-release.sh 0.1.10       # signs, then verifies with the pinned key
```

`sign-release.sh` xác minh kết quả của chính nó với `keys/camarelol.pub` trước
khi báo thành công, nên một chữ ký tạo bằng sai khoá không thể bị tải lên do
nhầm lẫn.

## Tiếp theo

- [Vì sao chỉ mục cũ bị từ chối](updates-rollback.md)
- [Tự đóng gói bản của riêng bạn](updates-packaging.md)
- [Khi nào bản cập nhật bị từ chối](updates-troubleshooting.md)
