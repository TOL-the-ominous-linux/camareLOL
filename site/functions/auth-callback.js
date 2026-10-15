/**
 * Google gửi mã về đây. Đổi mã lấy token, đặt cookie phiên, rồi về trang.
 *
 * Đây là chỗ duy nhất `GOOGLE_CLIENT_SECRET` được dùng. Nếu nó lọt ra khỏi máy
 * chủ thì toàn bộ lớp bảo vệ mất hết ý nghĩa, nên nó đọc từ biến môi trường và
 * không bao giờ được trả về cho trình duyệt — kể cả trong thông báo lỗi.
 */

import { createHmac, timingSafeEqual } from "node:crypto";
import { exchangeCode, sessionCookie, json } from "./_shared.js";

function required(name) {
  const value = process.env[name];
  if (!value) throw new Error(`${name} is not set`);
  return value;
}

function stateIsValid(state) {
  if (!state || !state.includes(".")) return false;
  const [nonce, mac] = state.split(".");
  const expected = createHmac("sha256", required("SESSION_SECRET"))
    .update(nonce).digest("base64url");
  const a = Buffer.from(mac || "");
  const b = Buffer.from(expected);
  return a.length === b.length && timingSafeEqual(a, b);
}

export default async (event) => {
  const params = new URLSearchParams(event.rawQuery || "");
  const code = params.get("code");
  const state = params.get("state");
  const error = params.get("error");

  const back = (where, extra = {}) => ({
    statusCode: 302,
    headers: { Location: where, "Cache-Control": "no-store", ...extra },
  });

  if (error) {
    return back(`/discuss.html?error=${encodeURIComponent(params.get("error_description") || error)}`);
  }
  if (!code) {
    return back("/discuss.html?error=" + encodeURIComponent("không có mã trả về từ Google"));
  }
  if (!stateIsValid(state)) {
    // Không còn state đúng nghĩa là yêu cầu không bắt nguồn từ nút trên
    // trang của ta. Nói thẳng thay vì đăng nhập bừa.
    return back("/discuss.html?error=" + encodeURIComponent("yêu cầu đăng nhập không hợp lệ, hãy thử lại"));
  }

  try {
    const session = await exchangeCode(code);
    const cookie = sessionCookie({
      sub: session.sub,
      name: session.name,
      email: session.email,
      picture: session.picture,
      token: session.token,
    });
    return back("/discuss.html", { "Set-Cookie": cookie });
  } catch (err) {
    // Không trả về err.message cho trình duyệt: nó có thể chứa đoạn thân phản
    // hồi từ Google. Chi tiết để trong nhật ký máy chủ.
    console.error("[auth-callback]", err.message);
    return back("/discuss.html?error=" + encodeURIComponent("không đăng nhập được"));
  }
};