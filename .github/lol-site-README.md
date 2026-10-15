# LOL — trạm trung gian cập nhật

Hai tệp này phục vụ tại:

```
https://tamkogay9876-code.github.io/camareLOL-mirror/latest.json
https://tamkogay9876-code.github.io/camareLOL-mirror/latest.json.sig
```

Trỏ camareLOL vào bằng:

```console
camarelol --check-update
```

`latest.json` là địa chỉ mặc định, nên lệnh trên không cần tham số.

## Nhánh này là bản dự phòng, mirror thật nằm ở kho riêng

Mirror được phục vụ là kho **`camareLOL-mirror`**, không phải nhánh này:

| | Nhánh `lol-site` (đây) | Kho `camareLOL-mirror` |
|---|---|---|
| Phục vụ tại | *không có* | `https://tamkogay9876-code.github.io/camareLOL-mirror` |
| Nội dung | hai tệp chỉ mục | hai tệp chỉ mục **và trang web** |
| Cập nhật khi có bản phát hành | có | có |

Nhánh này chỉ còn giữ như một đường dự phòng. Nếu Netlify lỡ tay, dựng một host
tĩnh khác trỏ vào nhánh này là có mirror trở lại — công cụ hỗ trợ cả Netlify,
Vercel, Cloudflare Pages và GitHub Pages.

## Nhánh được sinh ra tự động

Mỗi lần có bản phát hành mới, `lol-site.yml` chạy `lol_sync.py`, kiểm tra chữ
ký bằng khoá công khai trong kho mã nguồn, rồi ghi đè hai tệp. Đừng sửa tay tại
đây — lần chạy sau sẽ xoá.

Vì bản phát hành được đăng **trước** rồi mới ký, job phải chờ
`latest.json.sig` xuất hiện. Nếu thấy thông báo "signature not there yet",
đó là bình thường trong vài phút đầu.

## Không có gì đáng tin ở đây cả

Chỉ mục **không** phải thứ được tin. Binary camareLOL tự kiểm tra chữ ký bằng
khoá biên dịch sẵn vào nó. Sửa hai tệp ở đây thì mọi máy đều từ chối — đã
kiểm chứng bằng cách sửa `sha256` và `url` trên một trạm đang chạy.

Mirror không làm yếu đi niềm tin; nó chỉ giúp khi kho phát hành không trả lời.
Khoá vẫn nằm trong binary, nên mirror bị sửa cũng không đổi được điều đó.

Xem `lol/README.md` ở nhánh chính để biết trạm này làm được và không làm được
gì.