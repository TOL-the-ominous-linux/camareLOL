# Cập nhật camareLOL / Updating camareLOL

Trang này nói rõ bộ cập nhật làm gì, và — quan trọng hơn — **không** làm gì.

## Nguyên tắc

camareLOL xoá ổ đĩa. Một trình cập nhật tự thay thế chính nó là đường đi thẳng từ
"máy chủ bị chiếm" tới "dữ liệu bị xoá". Nên mọi quyết định ở đây đều thiên về
**không hành động**:

| | |
|---|---|
| Mặc định | Chỉ **báo cáo**. Không tải, không ghi gì. |
| Cài đặt | Phải có `--update` do người dùng gõ tay. |
| Khi đang cài đặt hệ thống | Từ chối, kể cả khi đã có `--update`. |
| Kiểm tra SHA-256 | Bắt buộc. Lệch thì dừng, binary cũ giữ nguyên. |
| Chạy thử bản mới | Bắt buộc `--version` phải chạy được, nếu không thì vứt đi. |
| Ghi đè | Dùng `rename()`, nguyên tử — không có khoảnh khắc nào binary bị nửa vời. |
| Hoàn tác | Bản cũ giữ ở `<tên>.prev`, `--rollback-update` để đưa lại. |

## Dùng

```console
camarelol --check-update          # chỉ hỏi, không làm gì (mã thoát 10 nếu có bản mới)
camarelol --update                # tải, kiểm tra, cài
camarelol --rollback-update       # đưa bản cũ trở lại
camarelol --update-url <URL>      # dùng chỉ mục khác (mirror, nội bộ)
```

Trong giao diện TUI, phím `u` ở màn hình chào kiểm tra và **hiện kết quả lên màn
hình**. Nó không bao giờ tự cài.

## Định dạng chỉ mục

`latest.json`:

```json
{
  "schema": 1,
  "version": "0.1.6",
  "url": "https://github.com/tamkogay9876-code/camareLOL/releases/download/v0.1.6/camarelol",
  "sha256": "…64 ký tự hex…",
  "size": 1839000,
  "notes": "ghi chú ngắn"
}
```

Mỗi lần phát hành, CI sinh file này từ chính binary nó vừa build và đính kèm vào
release. **Chưa** có bước tự đăng lên trang web: dùng `--update-url` trỏ tới nơi
bạn đã đặt file, hoặc tự host nó.

## Chữ ký

SHA-256 phát hiện **file hỏng hoặc tải thiếu**. Nó *không* phát hiện máy chủ độc
hại hay DNS bị chiếm — trong trường hợp đó, cả hash lẫn payload đều đến từ kẻ tấn
công, nên chúng khớp nhau một cách vô nghĩa.

Vì vậy chỉ mục còn được **ký tách rời** bằng GPG:

| | |
|---|---|
| Chữ ký kiểm tra trên | đúng những byte thô của chỉ mục, **trước khi** phân tích |
| Khoá tin cậy | `keys/camarelol.pub`, nhúng vào binary lúc build |
| Kiểm tra khoá | vân tay khoá ký phải **bằng** khoá đã ghim, không phải "khoá nào gpg tìm được" |
| Keyring của gpg | thư mục tạm, không đụng keyring thật của bạn |
| `EXPKEYSIG`, `BADSIG` | từ chối |

Chỉ mục không ký thì `--update` **từ chối cài**:

```console
$ camarelol --update
error: [update_signature_missing] refusing to install an update index that is not signed
  (no public key is configured, so the index cannot be authenticated)
  → publish latest.json.sig and configure a pinned key (see docs/UPDATES.md),
    or re-run with --allow-unsigned-update if you accept the risk
```

Chỉ khi bạn gõ `--allow-unsigned-update` thì nó mới đi tiếp. Đây là hành vi cố ý:
một bản cập nhật không ký thì không nên tự cài vào công cụ xoá ổ đĩa.

Xem `keys/README.md` để thiết lập khoá.

## Chống rollback

Chữ ký chứng minh **ai** ký, không chứng minh **cũ bao lâu**. Một trạm trung gian
bị chiếm có thể phục vụ lại một chỉ mục cũ *hợp lệ* để người dùng không nhận
được bản vá bảo mật.

Vì vậy chỉ mục mang thêm `expires` (mốc Unix), và binary từ chối chỉ mục quá hạn:

```console
$ camarelol --update
error: [update_rollback_blocked] the index expired ...
```

CI đặt hạn 7 ngày; trạm trung gian làm mới mỗi vài giờ nên không bao giờ chạm
ngưỡng. Tắt kiểm tra khi cần: `--allow-expired-index`.

Chỉ mục cũ hơn không có trường này vẫn được chấp nhận, để các trạm đang chạy
không hỏng ngay lập tức.

## Nếu bạn đóng gói camareLOL

Khi build từ mã nguồn — ebuild của Gentoo, PKGBUILD của Arch, hay gói của
chính bạn — **binary nên dùng kênh cập nhật của bạn, không phải của dự án
gốc**:

```console
$ CARAMELOL_UPDATE_URL="https://mirror.cua-ban.vn/camarelol/latest.json" cargo build --release
$ CAMAMELOL_NO_UPDATE=1 cargo build --release    # hoặc tắt hẳn
```

Vì sao:

- Cập nhật đi qua trình quản lý gói của bạn, không phải updater nhúng.
- Updater gốc sẽ đề nghị binary mà quản lý gói của bạn chưa build, chưa kiểm
  thử — đúng thứ bạn không muốn cài lên máy người dùng.

Hai biến được ghi vào **lúc build**, không đọc lúc chạy: giá trị lấy từ môi
trường khi khởi động có thể bị kẻ cài đặt thay đổi, và đó không phải phía nên
tin.

**Lưu ý:** đổi biến môi trường *sau* lần build đầu có thể không kích hoạt
build lại. Xoá `target/release/build/camarelol-*` hoặc build trong thư mục sạch.

## Vì sao không tự cập nhật

Không có cơ chế "khởi động thì tự lên bản mới" cho công cụ này, và có chủ ý:

- Bản cũ hơn có thể là bản bạn *đã kiểm chứng* trên đĩa thật.
- Tự thay binary khi đang cài là cách nhanh nhất để để lại hệ thống nửa vời.
- Người dùng không thể phân biệt "tôi đang chạy mã nào" nếu công cụ không nói.

Nếu bạn vẫn muốn tự động hoá toàn phần, hãy làm nó ở tầng quản lý gói (pacman
hook, systemd timer cập nhật `camarelol` từ kho của bạn), **không** làm trong
chính trình cài đặt.

## Máy chủ kiểm thử nội bộ

Vòng lặp http chỉ được phép trên loopback, để kiểm thử đầu-cuối với một mirror cục
bộ:

```console
python3 -m http.server 8766 --bind 127.0.0.1
camarelol --check-update --update-url http://127.0.0.1:8766/latest.json
```

Mọi thứ ngoài loopback đều phải là `https://`.