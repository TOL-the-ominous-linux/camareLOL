/**
 * Dùng chung cho các Netlify Function: phiên đã ký, và cách gọi GitHub.
 *
 * Phiên nằm trong cookie đã ký HMAC chứ không phải trong một cơ sở dữ liệu.
 * Một site tĩnh không có nơi nào để lưu, nên token phải đi cùng người dùng —
 * và cookie httpOnly+Secure+SameSite là cách làm chuẩn khi không có máy chủ
 * trạng thái. Đổi lại: đăng xuất là xoá cookie, không thu hồi token ở Google.
 */

import { createHmac, timingSafeEqual } from "node:crypto";

const GOOGLE_TOKEN = "https://oauth2.googleapis.com/token";
const GOOGLE_USERINFO = "https://openidconnect.googleapis.com/v1/userinfo";
const GITHUB_GRAPHQL = "https://api.github.com/graphql";

/** Bắt buộc có trong môi trường Netlify. Không có giá trị mặc định có tác dụng. */
function required(name) {
  const value = process.env[name];
  if (!value) {
    // Ném ra lỗi có nội dung rõ ràng. Một hàm thiếu biến môi trường mà im
    // lặng trả 200 là kiểu lỗi sẽ chỉ lộ ra khi ai đó đăng nhập thật.
    throw new Error(`${name} is not set`);
  }
  return value;
}

function b64url(input) {
  return Buffer.from(input).toString("base64url");
}

function sign(payload) {
  const body = b64url(JSON.stringify(payload));
  const mac = createHmac("sha256", required("SESSION_SECRET")).update(body).digest("base64url");
  return `${body}.${mac}`;
}

function unsign(cookie) {
  if (!cookie || !cookie.includes(".")) return null;
  const [body, mac] = cookie.split(".");
  if (!body || !mac) return null;

  const expected = createHmac("sha256", required("SESSION_SECRET"))
    .update(body).digest("base64url");

  // So sánh thời gian cố định: so sánh chuỗi bằng == dừng ngay ở ký tự khác
  // nhau, và đó là thứ đoán được qua thời gian.
  const a = Buffer.from(mac);
  const b = Buffer.from(expected);
  if (a.length !== b.length || !timingSafeEqual(a, b)) return null;

  try {
    const payload = JSON.parse(Buffer.from(body, "base64url").toString("utf8"));
    // Cookie sống 7 ngày; hết hạn thì chữ ký vẫn hợp lệ nhưng ý nghĩa thì không.
    if (!payload.exp || Date.now() / 1000 > payload.exp) return null;
    return payload;
  } catch {
    return null;
  }
}

export const COOKIE = "camarelol_session";
const WEEK = 7 * 24 * 60 * 60;

export function sessionCookie(payload) {
  const value = sign({ ...payload, exp: Math.floor(Date.now() / 1000) + WEEK });
  return `${COOKIE}=${value}; Path=/; Max-Age=${WEEK}; HttpOnly; Secure; SameSite=Lax`;
}

export function clearCookie() {
  return `${COOKIE}=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Lax`;
}

export function currentSession(event) {
  const header = event.headers.cookie || "";
  const match = header.split(/;\s*/).find((c) => c.startsWith(`${COOKIE}=`));
  return unsign(match ? match.slice(COOKIE.length + 1) : null);
}

/** Đổi mã một lần lấy token và thông tin người dùng từ Google. */
export async function exchangeCode(code) {
  const response = await fetch(GOOGLE_TOKEN, {
    method: "POST",
    headers: { "Content-Type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({
      code,
      client_id: required("GOOGLE_CLIENT_ID"),
      client_secret: required("GOOGLE_CLIENT_SECRET"),
      redirect_uri: required("GOOGLE_REDIRECT_URI"),
      grant_type: "authorization_code",
    }),
  });
  if (!response.ok) {
    throw new Error(`token exchange failed: ${response.status} ${await response.text()}`);
  }
  const { access_token: token } = await response.json();

  const info = await fetch(GOOGLE_USERINFO, {
    headers: { Authorization: `Bearer ${token}` },
  });
  if (!info.ok) {
    throw new Error(`userinfo failed: ${info.status}`);
  }
  const profile = await info.json();
  return {
    // sub là định danh ổn định của tài khoản; email có thể đổi.
    sub: profile.sub,
    name: profile.name,
    email: profile.email,
    picture: profile.picture,
    token,
  };
}

/**
 * Đăng một bài thảo luận.
 *
 * Dùng token bot của kho, không phải token của người đọc: họ đăng nhập bằng
 * Google, không phải bằng GitHub, nên họ không có token nào để cho.
 *
 * Đổi lại, bài viết mang tên Google của người viết trong phần đầu thân bài thay
 * vì tên GitHub. Đây là đánh đổi thật và nó được nói ra ở trang thảo luận chứ
 * không giấu trong mã nguồn.
 */
export async function createDiscussion({ title, body, authorName }) {
  const query = `
    mutation($repoId: ID!, $catId: ID!, $title: String!, $body: String!) {
      createDiscussion(input: {
        repositoryId: $repoId, categoryId: $catId, title: $title, body: $body
      }) { discussion { number url } }
    }`;

  const response = await fetch(GITHUB_GRAPHQL, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${required("GITHUB_BOT_TOKEN")}`,
      "Content-Type": "application/json",
      "User-Agent": "camarelol-discussions",
    },
    body: JSON.stringify({
      query,
      variables: {
        repoId: required("GITHUB_REPO_ID"),
        catId: required("GITHUB_CATEGORY_ID"),
        title: title.slice(0, 240),
        body: `${authorName} — ${body}`,
      },
    }),
  });

  const json = await response.json();
  if (json.errors) {
    throw new Error(`GitHub: ${json.errors.map((e) => e.message).join("; ")}`);
  }
  return json.data.createDiscussion.discussion;
}

/** Danh sách bài thảo luận mới nhất, cho trang đọc không cần đăng nhập. */
export async function listDiscussions() {
  const query = `
    query($repoId: ID!, $n: Int!) {
      repository(owner: "${required("GITHUB_OWNER")}", name: "${required("GITHUB_REPO")}") {
        discussions(first: $n, orderBy: { field: CREATED_AT, direction: DESC }) {
          nodes { number title url createdAt category { name } author { login } }
        }
      }
    }`;

  const response = await fetch(GITHUB_GRAPHQL, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${required("GITHUB_BOT_TOKEN")}`,
      "Content-Type": "application/json",
      "User-Agent": "camarelol-discussions",
    },
    body: JSON.stringify({ query, variables: { n: 30 } }),
  });
  const json = await response.json();
  if (json.errors) throw new Error(`GitHub: ${json.errors.map((e) => e.message).join("; ")}`);
  return json.data.repository.discussions.nodes;
}

/** JSON kèm tiêu đề chống đệm nhầm. */
export function json(statusCode, body, extra = {}) {
  return {
    statusCode,
    headers: {
      "Content-Type": "application/json; charset=utf-8",
      "Cache-Control": "no-store",
      ...extra,
    },
    body: JSON.stringify(body),
  };
}