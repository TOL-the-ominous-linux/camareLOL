'use strict';

/**
 * Cập nhật binary từ GitHub Releases.
 *
 * Đây là phần mà dự án đã cố ý bỏ đi ở bản 0.1.11, với lý do ghi trong tài
 * liệu: chương trình xoá ổ đĩa, còn một thành phần việc của nó là thay thế chính
 * nó thì là con đường ngắn nhất từ máy chủ bị chiếm tới dữ liệu bị xoá.
 *
 * Vì vậy bước tải ở đây không chỉ `curl` rồi `chmod`. Nó kiểm SHA-256 so với
 * tệp `SHA256SUMS` phát hành kèm.
 *
 * Nói thẳng điều này chống được và không chống được cái gì: nó bắt được tệp tải
 * về bị hỏng giữa đường, và bắt được máy chủ trả sai tệp. Nó *không* chống
 * được máy chủ bị chiếm — kẻ tấn công sửa được cả tệp kiểm lẫn tệp nhị phân.
 * Chữ ký mà bản cũ từng có đã bị gỡ cùng updater, và đây không phải chỗ để khôi
 * phục nó. Ai cài bản này thì chọn tin vào GitHub Releases, giống như mọi người.
 */

const crypto = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync, spawnSync } = require('node:child_process');

const REPO = 'TOL-the-ominous-linux/camareLOL';
const DOWNLOAD_BASE = `https://github.com/${REPO}/releases/download`;

function have(cmd) {
  return spawnSync('sh', ['-c', `command -v ${cmd}`], { encoding: 'utf8' }).status === 0;
}

/** Ghi ra một dòng, không phụ thuộc console có hay không. */
function say(line) {
  process.stdout.write(line + '\n');
}

/** Tải một tệp về `dest`. Dùng curl khi có, fetch không. */
function download(url, dest) {
  if (have('curl')) {
    const r = spawnSync('curl', ['-sSL', '--fail', '--max-time', '300', '-o', dest, url], {
      encoding: 'utf8',
    });
    if (r.status === 0) return true;
    say(`  tải bằng curl thất bại: ${(r.stderr || '').trim()}`);
  }

  const r = spawnSync('curl', ['-sSL', '--fail', '--max-time', '300', '-o', dest, url]);
  if (r.status === 0) return true;

  throw new Error('không tải được: cần curl để tải bản cập nhật');
}

/** sha256 của một tệp. */
function sha256(file) {
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

/** Đọc hash của `camarelol` trong tệp SHA256SUMS. */
function expectedFrom(sumsText) {
  for (const line of sumsText.split('\n')) {
    const m = line.match(/^([0-9a-f]{64})\s+\*?camarelol\s*$/);
    if (m) return m[1];
  }
  return null;
}

/** Nơi binary sẽ nằm sau khi cập nhật. */
function targetPath() {
  // Cùng thư mục với binary hiện tại: cập nhật tại chỗ, không cần cài lại gói.
  return process.env.CAMARELOL_BINARY || path.join(__dirname, '..', 'vendor', 'camarelol');
}

/**
 * Cập nhật binary lên `version`.
 *
 * Trả về true nếu binary trên đĩa đã là bản mới. Không ném lỗi: một bản cập
 * nhật hỏng không được phép biến thành việc không cài được hệ thống — người
 * dùng vẫn có binary cũ để chạy.
 */
async function selfUpdate(version) {
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'camarelol-up-'));
  const sumsPath = path.join(tmp, 'SHA256SUMS');
  const binPath = path.join(tmp, 'camarelol');
  const dest = targetPath();

  try {
    say(`Đang tải bản v${version}...`);
    download(`${DOWNLOAD_BASE}/v${version}/SHA256SUMS`, sumsPath);

    const expected = expectedFrom(fs.readFileSync(sumsPath, 'utf8'));
    if (!expected) {
      say('  Tệp SHA256SUMS không có dòng cho camarelol — dừng, không cài tệp chưa kiểm.');
      return false;
    }

    download(`${DOWNLOAD_BASE}/v${version}/camarelol`, binPath);

    const actual = sha256(binPath);
    if (actual !== expected) {
      say('  SHA-256 không khớp. Dừng, không cài.');
      say(`    tải về : ${actual}`);
      say(`    khai báo: ${expected}`);
      return false;
    }
    say(`  SHA-256 khớp (${actual.slice(0, 16)}…)`);

    fs.mkdirSync(path.dirname(dest), { recursive: true });
    fs.copyFileSync(binPath, dest);
    fs.chmodSync(dest, 0o755);
    say(`Đã cài ${dest}`);
    return true;
  } catch (err) {
    say(`  Cập nhật không thành công: ${err.message}`);
    return false;
  } finally {
    try {
      fs.rmSync(tmp, { recursive: true, force: true });
    } catch (_) {
      /* thư mục tạm còn lại không đáng để báo */
    }
  }
}

module.exports = { selfUpdate, targetPath, sha256, have, expectedFrom, download };

// Cho phép kiểm tay: `node lib/selfupdate.js 0.1.12`
if (require.main === module) {
  const v = process.argv[2];
  if (!v) {
    say('dùng: node lib/selfupdate.js <phiên bản>');
    process.exit(2);
  }
  selfUpdate(v).then((ok) => process.exit(ok ? 0 : 1));
}
