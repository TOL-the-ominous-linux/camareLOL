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
$ gpg --list-secret-keys --keyid-format long
$ gpg --armor --export <KEYID> > keys/camarelol.pub
```

Khoá riêng **không** được đưa vào kho mã nguồn. Lưu nó ở nơi an toàn và đưa
vào GitHub Actions secret:

- `CAMARELOL_SIGNING_KEY` — khoá riêng ở dạng ASCII-armored, lấy bằng
  `gpg --armor --export-secret-keys <KEYID>`

`CAMARELOL_SIGNING_KEY_ID` là tuỳ chọn. Bỏ trống thì CI tự lấy khoá duy nhất vừa
nhập, nên thường chỉ cần **một** secret. Chỉ đặt nó khi bạn nhập nhiều khoá.

CI sẽ ký `latest.json` thành `latest.json.sig` và tự kiểm tra lại chữ ký trước
khi phát hành.

## Khi chưa có khoá

Các bản phát hành hiện tại chưa có `keys/camarelol.pub`, nên chỉ mục đi kèm là
**không ký**. camareLOL sẽ **từ chối cài** các bản cập nhật đó, trừ khi bạn
truyền `--allow-unsigned-update`. Đó là hành vi đúng: một chỉ mục không ký thì
không chứng minh được điều gì.

Xem `docs/UPDATES.md`.
