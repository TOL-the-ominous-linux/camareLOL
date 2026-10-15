/**
 * Bắt đầu đăng nhập Google.
 *
 * Không có bí mật nào ở đây: `client_secret` chỉ được dùng khi đổi mã, và việc
 * đó chỉ xảy ra trong auth-callback — phía máy chủ.
 */

const GOOGLE_AUTH = "https://accounts.google.com/o/oauth2/v2/auth";

// Một `state` ngẫu nhiên chống việc kẹp session ID vào trang đích.
// Nó phải quay về đúng người dùng đó, nên nó ký ở đây và kiểm ở callback.
const { createHmac, randomBytes } = await import("node:crypto");

function required(name) {
  const value = process.env[name];
  if (!value) throw new Error(`${name} is not set`);
  return value;
}

export default async (event) => {
  const secret = required("SESSION_SECRET");
  const nonce = randomBytes(16).toString("base64url");
  const state = `${nonce}.${createHmac("sha256", secret).update(nonce).digest("base64url")}`;

  const url = new URL(GOOGLE_AUTH);
  url.searchParams.set("client_id", required("GOOGLE_CLIENT_ID"));
  url.searchParams.set("redirect_uri", required("GOOGLE_REDIRECT_URI"));
  url.searchParams.set("response_type", "code");
  url.searchParams.set("scope", "openid email profile");
  url.searchParams.set("state", state);
  // Luôn hỏi lại: im lặng dùng lại tài khoản đã chọn trước đó nghĩa là
  // không có nút "đăng nhập bằng tài khoản khác".
  url.searchParams.set("prompt", "select_account");

  return {
    statusCode: 302,
    headers: {
      Location: url.toString(),
      "Cache-Control": "no-store",
    },
  };
};