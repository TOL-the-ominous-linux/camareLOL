/**
 * Kiểm tra phần máy chủ. Chạy bằng Node trong CI.
 *
 *   node site/functions/test.js
 *
 * Tập trung vào thứ dễ sai và khó thấy bằng mắt: chữ ký cookie và việc một
 * cookie bị sửa phải bị từ chối. Phần đăng nhập thật cần Google, nên nó không
 * kiểm được ở đây và điều đó được nói ra thay vì giả vờ đã kiểm.
 */

import assert from "node:assert/strict";
import { createHmac } from "node:crypto";
import test from "node:test";

// Đặt ngoài functions/ có chủ đích: mọi tệp .js trong thư mục đó đều trở
// thành một endpoint của Netlify, kể cả tệp không phải hàm.

process.env.SESSION_SECRET = "test-secret-not-a-real-one";
process.env.GITHUB_OWNER = "tamkogay9876-code";
process.env.GITHUB_REPO = "camareLOL";

const shared = await import("./_shared.js");

function cookieFrom(response) {
  return (response.headers["Set-Cookie"] || "").split(";")[0];
}

test("a session round-trips", () => {
  const cookie = cookieFrom(shared.sessionCookie({ name: "An", email: "an@example.invalid" }));
  const session = shared.currentSession({ headers: { cookie } });
  assert.equal(session.name, "An");
  assert.equal(session.email, "an@example.invalid");
});

test("the cookie is httpOnly, Secure and SameSite", () => {
  const raw = shared.sessionCookie({ name: "An" }).headers["Set-Cookie"];
  for (const flag of ["HttpOnly", "Secure", "SameSite=Lax", "Path=/"]) {
    assert.ok(raw.includes(flag), `thiếu ${flag}: ${raw}`);
  }
});

test("a tampered cookie is refused", () => {
  const cookie = cookieFrom(shared.sessionCookie({ name: "An" }));
  const [body, mac] = cookie.split(".");
  // Đổi tên trong phần thân, giữ nguyên chữ ký.
  const swapped = Buffer.from(JSON.stringify({ name: "Kẻ giả" })).toString("base64url");
  assert.equal(shared.currentSession({ headers: { cookie: `${swapped}.${mac}` } }), null);
  assert.notEqual(body, swapped);
});

test("a cookie signed with another key is refused", () => {
  const original = process.env.SESSION_SECRET;
  const cookie = cookieFrom(shared.sessionCookie({ name: "An" }));
  process.env.SESSION_SECRET = "a-different-secret";
  try {
    assert.equal(shared.currentSession({ headers: { cookie } }), null);
  } finally {
    process.env.SESSION_SECRET = original;
  }
});

test("an expired cookie is refused", () => {
  const body = Buffer.from(JSON.stringify({
    name: "An", exp: Math.floor(Date.now() / 1000) - 10,
  })).toString("base64url");
  const mac = createHmac("sha256", process.env.SESSION_SECRET).update(body).digest("base64url");
  assert.equal(shared.currentSession({ headers: { cookie: `${body}.${mac}` } }), null);
});

test("no cookie, no session", () => {
  assert.equal(shared.currentSession({ headers: {} }), null);
  assert.equal(shared.currentSession({ headers: { cookie: "x=y; z=1" } }), null);
  assert.equal(shared.currentSession({ headers: { cookie: "camarelol_session=garbage" } }), null);
});

test("clearing the cookie expires it", () => {
  const raw = shared.clearCookie();
  assert.ok(raw.includes("Max-Age=0"), raw);
  assert.ok(raw.includes("HttpOnly"), raw);
});

test("the error helper never caches", () => {
  const response = shared.json(200, { ok: true });
  assert.equal(response.headers["Cache-Control"], "no-store");
  assert.ok(response.headers["Content-Type"].startsWith("application/json"));
});

test("a missing secret is refused loudly", () => {
  const original = process.env.SESSION_SECRET;
  delete process.env.SESSION_SECRET;
  try {
    // Không im lặng trả về phiên rỗng: sẽ biến thành "ai cũng đăng nhập được".
    assert.throws(() => shared.sessionCookie({ name: "An" }), /SESSION_SECRET is not set/);
  } finally {
    process.env.SESSION_SECRET = original;
  }
});
