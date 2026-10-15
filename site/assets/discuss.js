/* Trang thảo luận: đăng nhập bằng Google, đọc và đăng bài.
 *
 * Không có bí mật nào ở đây. Trình duyệt chỉ chuyển hướng và gọi /api/*; việc
 * đổi mã lấy token diễn ra ở function, nơi `client_secret` được giữ.
 *
 * Mọi nội dung do người dùng nhập được chèn bằng textContent, không phải
 * innerHTML. Một bài đăng là dữ liệu không tin cậy.
 */

const status = document.getElementById("status");
const list = document.getElementById("list");
const signin = document.getElementById("signin");
const composer = document.getElementById("composer");

function say(text, isError) {
  status.textContent = text;
  status.classList.toggle("bad", Boolean(isError));
}

function date(iso) {
  return new Date(iso).toISOString().slice(0, 10);
}

function renderList(items) {
  list.textContent = "";
  if (!items.length) {
    const li = document.createElement("li");
    li.className = "note";
    li.textContent = "chưa có bài nào. Hãy là người đầu tiên.";
    list.append(li);
    return;
  }
  for (const d of items) {
    const li = document.createElement("li");
    const a = document.createElement("a");
    a.href = d.url;
    a.target = "_blank";
    a.rel = "noopener noreferrer";
    a.textContent = d.title;
    const sub = document.createElement("div");
    sub.className = "sub";
    // `author` có thể là null nếu tài khoản GitHub đã bị xoá.
    sub.textContent = `${date(d.createdAt)} · ${d.category ? d.category.name : "—"}`
      + (d.author ? ` · ${d.author.login}` : "");
    li.append(a, sub);
    list.append(li);
  }
}

async function load() {
  try {
    const response = await fetch("/api/discussions", { credentials: "same-origin" });
    const data = await response.json();

    renderList(data.discussions || []);

    if (data.user) {
      signin.hidden = true;
      composer.hidden = false;
      document.getElementById("as-who").textContent = `Đang đăng nhập: ${data.user.name}`;
    } else {
      signin.hidden = false;
      composer.hidden = true;
    }
  } catch {
    say("Không tải được danh sách bài. Phần máy chủ có thể chưa được cấu hình.", true);
  }
}

composer?.addEventListener("submit", async (event) => {
  event.preventDefault();
  const button = composer.querySelector("button[type=submit]");
  const title = document.getElementById("title").value.trim();
  const body = document.getElementById("body").value.trim();

  if (title.length < 4 || body.length < 10) {
    say("Tiêu đề ít nhất 4 ký tự, nội dung ít nhất 10 ký tự.", true);
    return;
  }

  button.disabled = true;
  say("Đang đăng…");
  try {
    const response = await fetch("/api/discussions", {
      method: "POST",
      credentials: "same-origin",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ title, body }),
    });
    const data = await response.json();
    if (!response.ok) {
      say(data.error || "không đăng được", true);
      return;
    }
    say("Đã đăng. Mở bài trên GitHub để sửa hoặc xoá.");
    composer.reset();
    await load();
  } catch {
    say("Không liên hệ được máy chủ. Thử lại sau.", true);
  } finally {
    button.disabled = false;
  }
});

// Google trả về ?error=... khi người dùng từ chối hoặc cấu hình sai.
const failed = new URLSearchParams(location.search).get("error");
if (failed) {
  say(failed, true);
  history.replaceState(null, "", location.pathname);
}

load();