/**
 * Đăng xuất: xoá cookie phiên.
 *
 * Token Google đã cấp không thu hồi được từ đây — nó thuộc về người dùng và
 * Google quản lý nó. Cái duy nhất bị huỷ là phiên ở phía ta, và đó cũng là
 * điều duy nhất cần huỷ khi trình duyệt dùng chung máy.
 */

import { clearCookie } from "./_shared.js";

export default async () => ({
  statusCode: 302,
  headers: {
    Location: "/discuss.html",
    "Cache-Control": "no-store",
    "Set-Cookie": clearCookie(),
  },
});