<!-- Dịch từ docs/en/updates-packaging.md. Sửa bản gốc rồi dịch lại; đừng sửa bản này. -->
# Cập nhật: đóng gói bản riêng của bạn

[← Chỉ mục tài liệu](index.md)

Nếu bạn đóng gói camareLOL — một PKGBUILD, một ebuild, một AppImage, một
mirror riêng — thì có một quyết định quan trọng hơn tất cả những quyết định
còn lại.

**Một tệp nhị phân bạn phân phối không nên đưa cho người dùng các tệp nhị phân
của dự án gốc.**

## Vì sao

Trình cập nhật của dự án gốc sẽ đưa ra một tệp nhị phân mà trình quản lý gói của
bạn chưa từng build, chưa từng kiểm thử, và chưa từng ký theo quy trình của bản
phân phối bạn. Khi đó một người dùng cài từ kho của bạn lại đang chạy bản build
của người khác cho gói của người khác. Với một trình cài đặt có phân vùng ổ đĩa,
đó không phải là chi tiết vụn vặt.

Cách thay thế là làm cho mirror của bạn thành kênh phân phối, như vậy vẫn giữ
được kiểm tra chữ ký, tổng kiểm, hạn dùng, hoàn tác và dự phòng mirror — và chỉ
đổi mỗi địa chỉ.

## Đặt kênh lúc build

Hai biến, được đọc **khi tệp nhị phân được build**:

```console
$ CAMARELOL_UPDATE_URL="https://mirror.example.org/camarelol/latest.json" cargo build --release
$ CAMARELOL_NO_UPDATE=1 cargo build --release      # or switch the updater off
```

| biến | tác động |
|---|---|
| `CAMARELOL_UPDATE_URL` | chỉ mục mà tệp nhị phân của bạn yêu cầu |
| `CAMARELOL_NO_UPDATE` | không có kênh nào được nhúng cả; trình cập nhật từ chối chạy |

Những giá trị này được đóng cứng vào tệp nhị phân, không phải đọc từ môi trường
lúc khởi động. Một giá trị lấy từ môi trường có thể bị kẻ cài đặt gói đặt vào,
và đó không phải phía đáng tin.

## Chính tả

Các biến là `CAMARELOL_UPDATE_URL` và `CAMARELOL_NO_UPDATE`. Cả hai từng bị viết
ngược trong tài liệu — `CARAMELOL_` và `CAMAMELOL_` — và một biến viết sai sẽ bị
bỏ qua **một cách âm thầm**: build vẫn thành công, người đóng gói tin rằng
mirror của họ đã được cấu hình, và mọi tệp nhị phân họ phân phối lại lặng lẽ
cập nhật từ dự án gốc.

Bây giờ build sẽ nêu đúng biến mà nó thật sự đọc khi thấy một chính tả gần đúng:

```
warning: CARAMELOL_UPDATE_URL is set but this build reads CAMARELOL_UPDATE_URL.
The misspelled one is ignored, and the binary keeps the upstream update channel.
```

## Xác minh nó đã có hiệu lực

Đừng giả định. Tệp nhị phân biết nó được build với gì:

```console
$ camarelol --check-update
camareLOL 0.1.10 (tag v0.1.10, commit 8b765de)
index: https://mirror.example.org/camarelol/latest.json
binary: /usr/bin/camarelol
indexes to try: 2
answered by: https://mirror.example.org/camarelol/latest.json
trust: the index is signed by a pinned key
```

Kiểm tra cả ba hướng, không chỉ một:

| build | `--check-update` phải hiện |
|---|---|
| không biến nào | mirror của dự án |
| có `CAMARELOL_UPDATE_URL` | mirror của bạn |
| `CAMARELOL_NO_UPDATE=1` | không có địa chỉ nào cả |

Trường hợp thứ ba là trường hợp đáng kiểm thử theo cả hai hướng — đặt nó, rồi
bỏ đặt và build lại. Việc xoá biến phải đưa trở lại mặc định, chứ không phải để
lại giá trị cũ được đóng cứng bên trong.

## Build lại

Cargo chạy lại build script khi những thứ này thay đổi, nên đặt một biến trước
một lần build lại thật sự có hiệu lực trong lần build lại đó.

Nếu bạn không chắc, hãy build trong một cây làm việc sạch thay vì xoá tay các
phần của `target/`:

```console
$ cargo clean && cargo build --release
```

Xoá tay `target/release/build/<pkg>` sẽ phá liên kết fingerprint, và build có
thể báo là đã cập nhật trong khi thực ra không.

## Mirror của bạn phải phục vụ gì

```console
$ ./scripts/sign-release.sh 0.1.10
$ ls /var/www/camarelol/
latest.json  latest.json.sig
```

Phải có cả hai tệp, nếu không thì đó không phải là mirror. `latest.json.sig` được
tải lên sau khi bản phát hành đã tạo — chỉ mục không thể ký trước khi bản phát
hành tồn tại để trỏ tới — nên bất cứ thứ gì đang chờ một bản phát hành mới nên
chờ chữ ký thay vì thất bại ngay ở lần 404 đầu tiên.

## Tiếp theo

- [Cập nhật: ký](updates-signing.md)
- [Vì sao chỉ mục cũ bị từ chối](updates-rollback.md)
- [Khi nào bản cập nhật bị từ chối](updates-troubleshooting.md)
