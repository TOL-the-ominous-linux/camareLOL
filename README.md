# LOL — trạm trung gian cập nhật

Hai tệp này phục vụ tại:

```
https://<tên-miền-của-bạn>/latest.json
https://<tên-miền-của-bạn>/latest.json.sig
```

Trỏ chúng vào camareLOL bằng:

```console
camarelol --check-update --update-url https://<tên-miền-của-bạn>/latest.json
```

## Nhánh này được sinh ra tự động

Mỗi lần có bản phát hành mới, `lol-site.yml` chạy `lol_sync.py`, kiểm tra chữ
ký bằng khoá công khai trong kho mã nguồn, rồi ghi đè hai tệp. Đừng sửa tay
tại đây — lần chạy sau sẽ xoá.

## Trỏ host tĩnh vào nhánh này

| Dịch vụ | Cách làm |
|---|---|
| **Netlify** | Netlify → Add new site → Import an existing project → chọn repo, branch `lol-site`. Publish directory: `.` |
| **Vercel** | Vercel → Add New → Project → Import → chọn repo, Framework Preset: *Other*, Build command để trống, Output Directory `.` |
| **Cloudflare Pages** | Workers & Pages → Create → Pages → Connect to Git → repo, production branch `lol-site` |
| **GitHub Pages** | Settings → Pages → Deploy from a branch → `lol-site` / `/ (root)` |

Cả bốn đều chỉ cần biết: nhánh `lol-site`, thư mục phục vụ là thư mục gốc.

## Không có gì đáng tin ở đây cả

Chỉ mục **không** phải thứ được tin. Binary camareLOL tự kiểm tra chữ ký bằng khoá
biên dịch sẵn vào nó. Sửa hai tệp trên nhánh này thì mọi máy đều từ chối — đã
kiểm chứng bằng cách sửa `sha256` và `url` trên một trạm đang chạy.

Xem `lol/README.md` ở nhánh chính để biết trạm này làm được và không làm được
gì.