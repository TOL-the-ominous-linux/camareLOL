# LOL — trạm trung gian cho cập nhật camareLOL

Một URL ngắn, dễ gõ, không phải biết GitHub đang ở đâu:

```console
$ curl -sSL https://camarelol.github.io/latest.json
$ camarelol --check-update --update-url https://camarelol.github.io/latest.json
```

## Điều quan trọng nhất: trạm này **không** làm yếu đi bảo mật

Khoá ký nằm **trong binary camareLOL**, không nằm ở đây và không được tải từ
đây. Nên:

| Nếu kẻ xấu chiếm được LOL | Kết quả |
|---|---|
| Sửa `sha256` hay `url` trong chỉ mục | Chữ ký hỏng → binary từ chối **trước khi** tải gì |
| Đổi cả hai tệp sang bản khác | Vẫn phải có chữ ký của khoá đã ghim → bị từ chối |
| Chặn không cho tải | Chỉ gây thiếu bản cập nhật, không gây mất an toàn |

Đã kiểm chứng: sửa `sha256` và `url` trong file trên LOL → binary báo
`update_signature_invalid` và không hề đụng vào bản thân nó.

Vì vậy LOL **không cần khoá riêng**, không cần quyền ghi vào đâu, và không giữ
gì bí mật. Nó chỉ chép hai tệp đã được ký sẵn.

## Cái duy nhất trạm này làm hỏng được

**Rollback**: phục vụ một chỉ mục cũ *hợp lệ* đã ký. Người dùng không bị xâm
nhập, nhưng không nhận được bản vá bảo mật mới.

Cách chặn: cho chỉ mục thêm trường `expires`, và binary từ chối chỉ mục quá hạn.
Xem "Đề xuất nâng cấp" bên dưới — **chưa làm**, cần bạn duyệt.

## Chạy

```console
# Cách 1: một lần, sinh ra hai tệp tĩnh (đủ dùng cho Cloudflare Pages)
$ python3 lol/lol_sync.py --out lol/site

# Cách 2: chạy thành dịch vụ (cho VPS)
$ python3 lol/lol_server.py --host 127.0.0.1 --port 8080 --sync-on-start
```

`lol_sync.py` **kiểm tra chữ ký** trước khi phát hành, bằng đúng khoá công khai
trong `keys/camarelol.pub`. Chỉ mục không khớp thì không chép.

`lol_server.py` phục vụ:

| Đường dẫn | Ý nghĩa |
|---|---|
| `GET /latest.json` | chỉ mục cập nhật |
| `GET /latest.json.sig` | chữ ký tách rời |
| `GET /healthz` | còn tệp không (503 nếu trống) |
| `POST /_refresh` | đồng bộ lại từ GitHub, cần bearer token |

Chỉ hai tệp trong danh sách là đọc được; không có xử lý đường dẫn nào khác, nên
không có gì để dò đường dẫn tới. Token refresh so sánh bằng `hmac.compare_digest`
để không rò rỉ theo thời gian.

## Triển khai

`camarelol.github.io` là Cloudflare Pages — **chỉ phục vụ tệp tĩnh**, không chạy được
Python. Nên:

1. Cho GitHub Actions chạy `lol_sync.py` theo lịch (cron), rồi đẩy `lol/site`
   sang nhánh `gh-pages`.
2. Trỏ Pages vào nhánh đó.

Không cần máy chủ luôn bật, không tốn tiền, không có bề mặt tấn công. Nếu bạn
muốn nó **luôn sống** và tự đồng bộ theo yêu cầu, dùng `lol_server.py` trên một
VPS và đặt Cloudflare trước nó.

## Đề xuất nâng cấp — chờ bạn duyệt

1. **`expires` trong chỉ mục** — chặn rollback. Đánh đổi: máy offline lâu ngày
   sẽ không cập nhật được cho tới khi có mạng trở lại.
2. **Mirror thứ hai** — `camarelol.github.io-2`, client thử lần lượt. Tốn công gấp đôi.
3. **`camarelol --update-url` mặc định trỏ LOL** — tiện hơn, nhưng làm mirror
   thành điểm chết duy nhất nếu nó hỏng. Tôi **không** đề xuất.
4. **Làm `lol` chạy trong CI** để mỗi lần phát hành là tự động — hợp lý nhất.