<!-- Dịch từ docs/en/updates-rollback.md. Sửa bản gốc rồi dịch lại; đừng sửa bản này. -->
# Cập nhật: vì sao một chỉ mục cũ bị từ chối

[← Chỉ mục tài liệu](index.md)

Một chữ ký chứng minh **ai** đã ký. Nó không nói gì về **bao lâu trước đây**.

Lỗ hổng đó quan trọng hơn vẻ ngoài của nó. Một mirror bị xâm nhập có thể phát lại
một chỉ mục hoàn toàn hợp lệ, ký đúng, từ ba tháng trước. Mọi thứ về nó đều xác
minh được — chữ ký là thật, tổng kiểm khớp, schema là bản hiện hành. Và một
client chấp nhận nó sẽ âm thầm ngừng nhận các bản vá bảo mật.

Bản thân chữ ký không ngăn được điều này. Hạn dùng mới ngăn được.

## Trường `expires`

```json
{
  "schema": 1,
  "version": "0.1.10",
  "url": "https://…/camarelol",
  "sha256": "…",
  "expires": 1793299200
}
```

`expires` là một dấu thời gian Unix. CI đặt nó trước bảy ngày và tạo lại chỉ
mục theo lịch, nên một mirror không bao giờ ở sát giới hạn.

Một client từ chối chỉ mục đã quá hạn dùng:

```console
$ camarelol --update
error: [update_rollback_blocked] the index expired
```

Bảy ngày được chọn để một mirror im lặng trong vài giờ là không thấy thấp, còn
một mirror đứng yên một tuần thì có. Đây không phải một hạn mức chặt — nó là
một hạn mức.

## Tắt phép kiểm tra này đi

```console
$ camarelol --update --allow-expired-index
```

Cờ này tồn tại cho trường hợp bạn đang dùng một máy tách mạng hoặc chỉ thỉnh
thoảng kết nối và muốn tự quyết định khi nào cập nhật. Giống
`--allow-unsigned-update`, nó bắt buộc phải gõ tay.

## Chỉ mục cũ hơn

Một chỉ mục không có trường `expires` vẫn được chấp nhận. Từ chối nó sẽ làm hỏng
mọi client ngay khoảnh khắc trường đó được đưa vào, mà những client quan trọng
nhất chính là những cái đã được cài sẵn.

Đó là một đánh đổi có chủ đích: thiếu trường nghĩa là chỉ mục có trước lớp bảo
vệ, không phải là nó độc hại. Khi mọi mirror đều phát ra trường này, ngoại lệ
có thể bị bỏ — hiện tại nó vẫn còn để một mirror cũ không bỏ rơi ai cả.

## Mirror không làm suy yếu lòng tin

Hỏi rằng thêm mirror có mở một lỗ hổng không là hợp lý. Không.

Khoá ký nằm bên trong tệp nhị phân, không nằm ở mirror và không nằm trong tệp
cấu hình nào. Một mirror bị sửa — để đổi URL, để trỏ sang nơi khác, để phục vụ
một chỉ mục cũ — sẽ tạo ra nội dung mà client từ chối. Không có mirror nào có
thể cho phép điều gì.

Một mirror có thể làm là trì hoãn bạn: phục vụ nội dung cũ, hoặc từ chối trả lời.
Đó là lý do:

- mọi chỉ mục đều được ký, nên nội dung luôn được kiểm tra bất kể ai phục vụ;
- `expires` giới hạn độ cũ tối đa của nội dung cũ, nên một mirror không thể ghim
  bạn vào một bản cũ vô hạn định;
- các mirror được thử **theo thứ tự**, và danh sách được nhúng lúc build, nên
  một bản sao bị can thiệp trên ổ đĩa không thể thêm hay đảo thứ tự chúng.

Một mirror chết sẽ bị bỏ qua và mirror kế tiếp được thử. Mất toàn bộ mirror
nghĩa là không cập nhật được — đó là sự cố, không phải xâm nhập.

## Tiếp theo

- [Cập nhật: ký](updates-signing.md)
- [Tự đóng gói bản của riêng bạn](updates-packaging.md)
- [Khi nào bản cập nhật bị từ chối](updates-troubleshooting.md)
