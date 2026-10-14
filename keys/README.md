# Khoá công khai ký chỉ mục cập nhật / Update-signing keys

Đặt khoá công khai GPG của bạn vào `keys/camarelol.pub`. Khi có file này,
`build.rs` **nhúng nó vào chính binary**, nên khoá được tin cậy là thứ nằm
trong binary chứ không phải thứ nằm cạnh chỉ mục.

Đây là điểm mấu chốt: nếu đọc khoá từ file đi kèm chỉ mục thì kẻ nào phục vụ
chỉ mục cũng chọn được khoá xác nhận cho chính nó — kiểm tra trở nên vô nghĩa.

## Tạo cặp khoá

**Trên máy riêng của bạn**, không phải trên máy đã dùng để build:

```console
$ gpg --full-generate-key
  → 10  (ECC, chỉ ký — không cần mã hoá, ta chỉ ký)
  → kích thước: Enter (mặc định 255)
  → hạn: 0 rồi Enter (không hết hạn; thay đổi bằng 2y nếu bạn thích)
  → tên: camareLOL release
  → email: của bạn
  → mật khẩu: xem ghi chú bên dưới
```

Sau đó:

```console
$ gpg --list-secret-keys --keyid-format long          # tìm mã khoá
$ gpg --armor --export <KEYID> > keys/camarelol.pub  # commit file này
$ gpg --armor --export-secret-keys <KEYID> > ~/camarelol-signing.asc
```

**Về mật khẩu khoá.** Đặt mật khẩu thì an toàn hơn cho file khoá riêng nếu nó lọt
lên máy. Nhưng khoá riêng cuối cùng vẫn phải nằm trong GitHub secret, nơi không
có mật khẩu bảo vệ thật sự — mật khẩu chỉ là một lớp nữa, không phải rào cản.
Nếu bạn đặt, nhớ tạo thêm secret `CAMARELOL_SIGNING_PASSPHRASE`.

Khoá riêng **không** được đưa vào kho mã nguồn. Lưu nó ở nơi an toàn và đưa
vào GitHub Actions secret:

- `CAMARELOL_SIGNING_KEY` — khoá riêng ở dạng ASCII-armored, lấy bằng
  `gpg --armor --export-secret-keys <KEYID>`

`CAMARELOL_SIGNING_KEY_ID` là tuỳ chọn. Bỏ trống thì CI tự lấy khoá duy nhất vừa
nhập, nên thường chỉ cần **một** secret. Chỉ đặt nó khi bạn nhập nhiều khoá.

Nếu bạn đặt mật khẩu cho khoá, thêm secret thứ hai:

- `CAMARELOL_SIGNING_PASSPHRASE` — mật khẩu khoá riêng

Bỏ trống secret này thì khoá phải được tạo **không** mật khẩu.

CI sẽ ký `latest.json` thành `latest.json.sig` và tự kiểm tra lại chữ ký trước
khi phát hành.

## Khi chưa có khoá

Các bản phát hành hiện tại chưa có `keys/camarelol.pub`, nên chỉ mục đi kèm là
**không ký**. camareLOL sẽ **từ chối cài** các bản cập nhật đó, trừ khi bạn
truyền `--allow-unsigned-update`. Đó là hành vi đúng: một chỉ mục không ký thì
không chứng minh được điều gì.

Xem `docs/UPDATES.md`.
