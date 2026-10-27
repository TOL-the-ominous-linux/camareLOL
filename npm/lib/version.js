'use strict';

/**
 * So sánh phiên bản.
 *
 * Bản trên npm có thể cũ hơn bản trên GitHub Releases. Vấn đề là biết điều đó
 * cũng không đủ: phải gọi được GitHub, phải hiểu được câu trả lời, và phải
 * không hỏng khi máy chưa có mạng — vì lúc đó thì người dùng cũng chưa nối
 * được Wi-Fi để cài hệ thống.
 */

/** So sánh hai chuỗi phiên bản kiểu semver rút gọn (0.1.12). */
function compare(a, b) {
  const pa = String(a).split('.').map((n) => parseInt(n, 10) || 0);
  const pb = String(b).split('.').map((n) => parseInt(n, 10) || 0);
  for (let i = 0; i < 3; i++) {
    const d = (pa[i] || 0) - (pb[i] || 0);
    if (d !== 0) return d < 0 ? -1 : 1;
  }
  return 0;
}

/** Bóc `v0.1.12` ra `0.1.12`. */
function normalise(tag) {
  return String(tag || '').replace(/^v/, '');
}

/**
 * Hỏi GitHub bản mới nhất.
 *
 * Trả về null thay vì ném lỗi: không có mạng là một tình huống bình thường ở
 * đúng thời điểm này, không phải sự cố.
 */
async function latestRelease(timeoutMs = 8000) {
  const ac = new AbortController();
  const timer = setTimeout(() => ac.abort(), timeoutMs);
  try {
    const res = await fetch('https://api.github.com/repos/TOL-the-ominous-linux/camareLOL/releases/latest', {
      signal: ac.signal,
      headers: { Accept: 'application/vnd.github+json', 'User-Agent': 'camarelol-npm' },
    });
    if (!res.ok) return null;
    const body = await res.json();
    if (!body || body.draft || !body.tag_name) return null;
    return {
      version: normalise(body.tag_name),
      tag: body.tag_name,
      url: body.html_url,
    };
  } catch (_err) {
    return null;
  } finally {
    clearTimeout(timer);
  }
}

module.exports = { compare, normalise, latestRelease };
