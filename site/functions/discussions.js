/**
 * Ai đang đăng nhập, và danh sách bài thảo luận.
 *
 * GET  → { user, discussions }
 * POST → tạo bài mới. Cần đăng nhập.
 *
 * Đọc được mà không cần đăng nhập: có người chỉ muốn đọc.
 */

import { currentSession, createDiscussion, listDiscussions, json } from "./_shared.js";

const MAX_BODY = 8000;

export default async (event) => {
  const session = currentSession(event);
  const user = session
    ? { name: session.name, email: session.email, picture: session.picture }
    : null;

  try {
    const discussions = await listDiscussions();
    if (event.httpMethod === "GET") {
      return json(200, { user, discussions });
    }

    if (event.httpMethod !== "POST") {
      return json(405, { error: "method not allowed" }, { Allow: "GET, POST" });
    }

    if (!session) {
      return json(401, { error: "cần đăng nhập để đăng bài" });
    }

    const body = JSON.parse(event.body || "{}");
    const title = String(body.title || "").trim();
    const text = String(body.body || "").trim();

    if (title.length < 4) {
      return json(400, { error: "tiêu đề quá ngắn" });
    }
    if (text.length < 10) {
      return json(400, { error: "nội dung quá ngắn" });
    }
    if (text.length > MAX_BODY) {
      return json(400, { error: `nội dung dài quá ${MAX_BODY} ký tự` });
    }
    if (/!\[.*\]\(javascript:/i.test(text)) {
      return json(400, { error: "liên kết không hợp lệ" });
    }

    const discussion = await createDiscussion({
      title, body: text, authorName: session.name || session.email || "vô danh",
    });
    return json(201, { user, discussion });
  } catch (err) {
    console.error("[discussions]", err.message);
    return json(502, { error: "không liên hệ được máy chủ GitHub, thử lại sau" });
  }
};