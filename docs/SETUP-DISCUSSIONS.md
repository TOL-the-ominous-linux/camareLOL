# Cấu hình trang thảo luận có đăng nhập

Tài liệu cho người duy trì. Người đọc bình thường không cần đọc tệp này.

> **Chưa chạy.** Trang thảo luận hiện trỏ sang GitHub Discussions. Phần đăng
> nhập dưới đây đã viết xong nhưng chưa được bật: nơi phục vụ trang tĩnh
> (GitHub Pages) không chạy được mã máy chủ, và Netlify đã hết hạn mức tài
> khoản miễn phí.

Trang thảo luận dùng hai phần: trang tĩnh từ kho `camareLOL`, và bốn hàm máy chủ
lấy từ `site/functions/` trong cùng kho đó. Bài viết vẫn nằm trong
GitHub Discussions.

```
https://tamkogay9876-code.github.io/camareLOL-mirror/discuss.html   ← giao diện
https://tamkogay9876-code.github.io/camareLOL-mirror/api/*           ← bốn function
```

## Vì sao cần phần máy chủ

`client_secret` của Google nằm trong HTML hoặc JavaScript nghĩa là ai cũng đọc
được, và ai đọc được đều mạo danh được ứng dụng. Trang tĩnh không có chỗ để
giữ nó. Vì vậy:

| việc | ở đâu |
|---|---|
| chuyển hướng tới Google | trình duyệt |
| **đổi mã lấy token** | **function** — nơi duy nhất dùng `client_secret` |
| giữ phiên | cookie đã ký HMAC |
| đọc và ghi bài | function, gọi GitHub GraphQL |

## 1. Google Cloud Console

1. Một project (dùng lại project cũ `dev-block-510510` cũng được, hoặc tạo mới).
2. **APIs & Services → Credentials → Create Credentials → OAuth client ID**
3. Loại **Web application**.
4. **Authorized redirect URIs** — đúng một dòng, không có dấu `/` cuối:

   ```
   https://tamkogay9876-code.github.io/camareLOL-mirror/api/auth-callback
   ```

5. **Authorized JavaScript origins**:

   ```
   https://tamkogay9876-code.github.io/camareLOL-mirror
   ```

### Thu hồi client secret cũ

Một client secret đã bị lộ trong một cuộc trò chuyện. **Xoá client đó**, đừng dùng
lại — kể cả khi bạn cho rằng nó chỉ nằm trong lịch sử tin nhắn:

- Xoá trong Credentials, hoặc
- vào **APIs & Secrets → OAuth consent screen → Publishing status** bấm
  **Withdraw** nếu project chỉ dùng cho việc này.

Cách nào cũng được, nhưng phải làm một trong hai trước khi tạo client mới.

Secret cũ **không** được viết vào tệp nào trong kho này. `scripts/check_secrets.py`
quét từng commit và sẽ chặn bản dựng — kể cả trong tài liệu.

## 2. Biến môi trường trên Netlify

Đặt bằng cơ chế secret của nơi chạy hàm, **không** đặt trong kho và không
đặt trong tệp cấu hình nào được theo dõi.

| biến | lấy ở đâu |
|---|---|
| `GOOGLE_CLIENT_ID` | Google Cloud Console |
| `GOOGLE_CLIENT_SECRET` | Google Cloud Console |
| `GOOGLE_REDIRECT_URI` | `https://tamkogay9876-code.github.io/camareLOL-mirror/api/auth-callback` |
| `SESSION_SECRET` | tự sinh, xem bên dưới |
| `GITHUB_BOT_TOKEN` | token của bot, xem bên dưới |
| `GITHUB_REPO_ID` | GraphQL node id của kho |
| `GITHUB_CATEGORY_ID` | node id của category "General" |
| `GITHUB_OWNER` | `tamkogay9876-code` |
| `GITHUB_REPO` | `camareLOL` |

Sinh `SESSION_SECRET`:

```console
$ openssl rand -base64 48
```

Lấy hai node id:

```console
$ curl -sS -H "Authorization: Bearer $GITHUB_BOT_TOKEN" \
    https://api.github.com/graphql -d '{"query":"{repository(owner:\"tamkogay9876-code\",name:\"camareLOL\"){id discussionCategories(first:10){nodes{id name}}}}"}'
```

## 3. Token của bot

Bài viết được đăng bằng token này, không phải token của người đọc — họ đăng
nhập bằng Google nên không có token GitHub nào.

Tạo **fine-grained personal access token**, giới hạn:

- Repository: chỉ `camareLOL`
- Permissions: **Discussions → Read and write**
- Không cần quyền khác

Token này thuộc về tài khoản bot. Nếu tài khoản của bạn là người duy trì duy
nhất, bài viết sẽ mang tên bot, không phải tên bạn.

## 4. Đánh đổi đã biết

Bài viết mang tên **Google** của người viết, do bot đăng. Người đọc sẽ thấy
tên Google ở đầu thân bài chứ không phải tên GitHub.

Đổi lại, cần một tài khoản đăng nhập bằng GitHub để giữ đúng tên tác giả. Đó
là một hệ thống đăng nhập thứ hai, và tôi chọn không làm cho tới khi có lý do.

Trang thảo luận nói rõ điều này với người đọc — nó không nằm im trong mã nguồn.

## 5. Kiểm tra

Sau khi cấu hình xong:

```console
$ curl -sS https://tamkogay9876-code.github.io/camareLOL-mirror/api/discussions | head -c 200
```

Báo danh sách bài thảo luận là đúng. Nếu báo `không liên hệ được máy chủ
GitHub`, kiểm tra `GITHUB_BOT_TOKEN`.

Mở `https://tamkogay9876-code.github.io/camareLOL-mirror/discuss.html`, bấm đăng nhập, và kiểm tra
vòng về trang có giữ phiên.

## Khi có sự cố

| triệu chứng | nguyên nhân thường gặp |
|---|---|
| `redirect_uri_mismatch` | URI trong Google khác dấu `/` cuối, hoặc khác `http`/`https` |
| `SESSION_SECRET is not set` | thiếu biến môi trường trên Netlify |
| đăng nhập xong không đăng nhập | cookie `Secure` bị chặn khi xem qua `http://` |
| không đăng được bài | token bot thiếu quyền Discussions |

Nhật ký hàm nằm ở Netlify: **Functions → … → Logs**. Chi tiết lỗi chỉ được ghi
đó, không trả về trình duyệt — thông báo lỗi từ Google có thể chứa dữ liệu của
người dùng.