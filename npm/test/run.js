#!/usr/bin/env node
'use strict';

/**
 * Kiểm gói trước khi phát hành.
 *
 *   node test/run.js
 *
 * Không chạm mạng và không hỏi gì.
 *
 * Tuần tự, không song song: một số test (phần prompt) sửa `process.stdin.isTTY`
 * rồi trả lại. Chạy chúng cùng lúc là hai test tranh cùng một biến toàn cục,
 * và cái thất bại là cái nào bị đổi tên dưới chân — loại lỗi mà bài kiểm tra
 * sinh ra để chống lại.
 */

const assert = require('node:assert');
const path = require('node:path');
const fs = require('node:fs');
const os = require('node:os');

const { compare, normalise } = require('../lib/version');
const { expectedFrom, sha256 } = require('../lib/selfupdate');
const wifi = require('../lib/wifi');

const tests = [];
function test(name, fn) {
  tests.push({ name, fn });
}

/* -------------------------------------------------------------------- wifi */

// Lỗi thật đo ngày 2026-10-17: một lần thử duy nhất với --max-time 5 báo
// "không có mạng" trên máy đang có mạng, vì lần khởi động lạnh chậm hơn 5
// giây. Người dùng bị hỏi nối Wi-Fi khi đã có mạng.

test('có mạng: đích đầu thành công thì thôi, không thử tiếp', () => {
  const asked = [];
  const ok = wifi.hasInternet((url) => {
    asked.push(url);
    return 0;
  });
  assert.strictEqual(ok, true);
  assert.strictEqual(asked.length, 1);
});

test('có mạng: đích đầu hỏng thì thử đích sau', () => {
  const asked = [];
  const ok = wifi.hasInternet((url) => {
    asked.push(url);
    return asked.length >= 2 ? 0 : 6; // 6 = không giải được tên
  });
  assert.strictEqual(ok, true);
  assert.ok(asked.length >= 2, `phải thử nhiều đích, mới hỏi ${asked.length}`);
});

test('có mạng: mọi đích hỏng thì mới báo không có', () => {
  let calls = 0;
  const ok = wifi.hasInternet(() => {
    calls += 1;
    return 7; // không kết nối được
  });
  assert.strictEqual(ok, false);
  assert.ok(calls >= 3, `phải thử hết danh sách đích, mới hỏi ${calls}`);
});

/* ------------------------------------------------------------------ version */

test('so sánh phiên bản: mới hơn', () => {
  assert.strictEqual(compare('0.1.12', '0.1.11'), 1);
  assert.strictEqual(compare('0.2.0', '0.1.99'), 1);
  assert.strictEqual(compare('1.0.0', '0.9.9'), 1);
});

test('so sánh phiên bản: cũ hơn', () => {
  assert.strictEqual(compare('0.1.11', '0.1.12'), -1);
  assert.strictEqual(compare('0.1.9', '0.1.10'), -1);
});

test('so sánh phiên bản: bằng nhau', () => {
  assert.strictEqual(compare('0.1.12', '0.1.12'), 0);
  assert.strictEqual(compare('0.1.12', 'v0.1.12'), 0);
  assert.strictEqual(compare('0.1', '0.1.0'), 0);
});

test('bóc thẻ v thành số', () => {
  assert.strictEqual(normalise('v0.1.12'), '0.1.12');
  assert.strictEqual(normalise('0.1.12'), '0.1.12');
  assert.strictEqual(normalise(undefined), '');
});

/* ------------------------------------------------------------- self update */

test('đọc hash camarelol trong SHA256SUMS', () => {
  const sums = [
    'aaa  camarelol-0.1.12.tar.gz',
    '5c749428e83dc93908f19c1d7570aa3302b786bd8467b6503042cbc2b7e9b3d9  camarelol',
    'ccc  PKGBUILD',
  ].join('\n');
  assert.strictEqual(
    expectedFrom(sums),
    '5c749428e83dc93908f19c1d7570aa3302b786bd8467b6503042cbc2b7e9b3d9',
  );
});

test('đọc hash khi tệp dùng dấu * (nhị phân)', () => {
  const sums = '5c749428e83dc93908f19c1d7570aa3302b786bd8467b6503042cbc2b7e9b3d9 *camarelol';
  assert.strictEqual(expectedFrom(sums), '5c749428e83dc93908f19c1d7570aa3302b786bd8467b6503042cbc2b7e9b3d9');
});

test('SHA256SUMS không có camarelol thì trả null — không đoán', () => {
  assert.strictEqual(expectedFrom('abc  something-else'), null);
  assert.strictEqual(expectedFrom(''), null);
});

test('bản đóng gói khớp SHA256SUMS đang phát hành', () => {
  const bin = path.join(__dirname, '..', 'vendor', 'camarelol');
  const sums = path.join(__dirname, '..', 'SHA256SUMS');
  if (!fs.existsSync(sums)) return; // chưa đính kèm, không phải lỗi
  const expected = expectedFrom(fs.readFileSync(sums, 'utf8'));
  assert.ok(expected, 'SHA256SUMS không có dòng cho camarelol');
  assert.strictEqual(sha256(bin), expected, 'binary trong gói không khớp checksum');
});

test('sha256 của một tệp thật', () => {
  const f = path.join(os.tmpdir(), `camarelol-test-${process.pid}`);
  fs.writeFileSync(f, 'abc');
  assert.strictEqual(
    sha256(f),
    'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad',
  );
  fs.unlinkSync(f);
});

/* ------------------------------------------------------------------ gói */

test('package.json khai báo bin camarelol', () => {
  const pkg = require('../package.json');
  assert.ok(pkg.bin && pkg.bin.camarelol, 'thiếu bin.camarelol');
  assert.ok(fs.existsSync(path.join(__dirname, '..', pkg.bin.camarelol)),
    `tệp bin không có: ${pkg.bin.camarelol}`);
});

test('mọi tệp lib được require được', () => {
  for (const f of fs.readdirSync(path.join(__dirname, '..', 'lib'))) {
    if (!f.endsWith('.js')) continue;
    require(path.join(__dirname, '..', 'lib', f));
  }
});

test('gói khai báo đúng những gì có trong đĩa', () => {
  const pkg = require('../package.json');
  for (const entry of pkg.files) {
    assert.ok(fs.existsSync(path.join(__dirname, '..', entry)), `files khai báo "${entry}" không tồn tại`);
  }
});

test('postinstall không in gì khi bị require', () => {
  // Nếu postinstall chạy lúc require, bài kiểm này sẽ in ra báo cáo trước
  // khi kết quả kịp hiện ra — đúng cái nó đã làm một lần.
  require('../lib/postinstall');
});

/* ---------------------------------------------------------------- prompt */

/**
 * `rl.close()` bắn 'close' ngay lập tức và giải promise với null. Nếu gọi nó
 * trước khi quyết định, promise đã có giá trị null và câu trả lời của người
 * dùng bị bỏ — `y` biến thành "không ai trả lời", tức là bỏ qua cập nhật.
 *
 * Không phải suy đoán: nó xảy ra. Chạy `camarelol` qua pty, gõ `y`, và không
 * tải gì cả.
 */
test('ask trả về đúng câu trả lời, không phải null', async () => {
  const { ask } = require('../lib/prompt');
  const readline = require('node:readline');

  const savedTTY = [process.stdin.isTTY, process.stdout.isTTY];
  const savedCreate = readline.createInterface;
  process.stdin.isTTY = true;
  process.stdout.isTTY = true;

  let onQuestion = null;
  readline.createInterface = () => ({
    question(q, cb) { onQuestion = cb; },
    close() { if (this.onClose) this.onClose(); },
    on(event, fn) { if (event === 'close') this.onClose = fn; },
  });

  try {
    const p1 = ask('> ');
    onQuestion('y');
    assert.strictEqual(await p1, true, 'gõ y phải cho true, không phải null');

    const p2 = ask('> ');
    onQuestion('n');
    assert.strictEqual(await p2, false, 'gõ n phải cho false');

    const p3 = ask('> ');
    onQuestion('');
    assert.strictEqual(await p3, false, 'Enter trống nghĩa là không');

    const p4 = ask('> ');
    onQuestion('đại loại gì đó');
    assert.strictEqual(await p4, null, 'trả lời không đọc được thì không đoán');
  } finally {
    readline.createInterface = savedCreate;
    process.stdin.isTTY = savedTTY[0];
    process.stdout.isTTY = savedTTY[1];
  }
});

test('không tương tác thì ask trả null, không chờ mãi', async () => {
  const { ask } = require('../lib/prompt');
  const saved = [process.stdin.isTTY, process.stdout.isTTY];
  process.stdin.isTTY = false;
  process.stdout.isTTY = false;
  try {
    assert.strictEqual(await ask('> '), null);
  } finally {
    process.stdin.isTTY = saved[0];
    process.stdout.isTTY = saved[1];
  }
});

/* ------------------------------------------------------------------ kết */

(async () => {
  let passed = 0;
  const failures = [];
  for (const { name, fn } of tests) {
    try {
      await fn();
      passed++;
      process.stdout.write(`  ok   ${name}\n`);
    } catch (err) {
      failures.push(name);
      process.stdout.write(`  FAIL ${name}\n       ${err.message}\n`);
    }
  }
  process.stdout.write(`\n  ${passed} đạt, ${failures.length} hỏng\n`);
  process.exit(failures.length ? 1 : 0);
})();
