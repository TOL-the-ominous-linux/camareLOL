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
| **Vercel** | Xem bên dưới — dự án đã có sẵn `vercel.json` |
| **Netlify** | Netlify → Add new site → Import an existing project → repo, branch `lol-site`. Publish directory: `.` |
| **Cloudflare Pages** | Workers & Pages → Create → Pages → Connect to Git → repo, production branch `lol-site` |
| **GitHub Pages** | Settings → Pages → Deploy from a branch → `lol-site` / `/ (root)` |

## Vercel

`vercel.json` nằm sẵn trong nhánh này: không framework, không build, phục vụ
tĩnh, kèm header cache và `nosniff`.

1. Vercel → **Add New → Project** → **Import** kho `tamkogay9876-code/camareLOL`
2. **Framework Preset**: *Other* (đã khai báo trong `vercel.json`, nhưng chọn
   *Other* cho chắc)
3. **Build Command**: để trống — không có gì để build
4. **Output Directory**: `.`
5. **Install Command**: để trống

Xong thì vào **Settings → Git → Production Branch** và đổi sang `lol-site`.
**Bước này quan trọng**: mặc định Vercel lấy `main`, mà `main` chứa mã nguồn
trình cài đặt chứ không phải mirror. Đổi sang `lol-site` thì mỗi lần có bản phát
hành mới, Vercel tự deploy lại.

Đợi lần deploy đầu tiên xong, URL sẽ có dạng `https://camareLOL.vercel.app`.
Kiểm tra:

```console
$ curl -sS https://camareLOL.vercel.app/latest.json | head -2
$ curl -sSI https://camareLOL.vercel.app/latest.json | head -4
```

## Không có gì đáng tin ở đây cả

Chỉ mục **không** phải thứ được tin. Binary camareLOL tự kiểm tra chữ ký bằng khoá
biên dịch sẵn vào nó. Sửa hai tệp trên nhánh này thì mọi máy đều từ chối — đã
kiểm chứng bằng cách sửa `sha256` và `url` trên một trạm đang chạy.

Xem `lol/README.md` ở nhánh chính để biết trạm này làm được và không làm được
gì.