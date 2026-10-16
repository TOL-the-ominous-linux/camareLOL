#!/usr/bin/env node
'use strict';

/**
 * camarelol — điểm vào do npm đặt trên PATH.
 *
 * Việc của tệp này chỉ là bốn việc, rồi chuyển quyền cho binary thật:
 *
 *   1. `--version` / `-V` / `--V` — in phiên bản và ra.
 *   2. So bản trên npm với bản trên GitHub Releases. Nếu cũ hơn, hỏi có
 *      cập nhật không.
 *   3. Nếu bỏ qua, và máy chưa có mạng, dẫn qua nối Wi-Fi.
 *   4. Chạy binary Rust.
 *
 * Bước 2 và 3 là của npm, không phải của camareLOL. Tài liệu `update.md` nói
 * rõ binary không tự cập nhật — điều đó vẫn đúng. Thứ tự để người dùng quyết
 * định tồn tại ở đây, trước khi binary có mặt trên đĩa.
 */

const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const { ask, interactive } = require('../lib/prompt');
const { compare, latestRelease } = require('../lib/version');
const { selfUpdate, targetPath } = require('../lib/selfupdate');
const { connectWifi } = require('../lib/wifi-flow');
const { hasInternet, say } = require('../lib/wifi');

// Bản này đóng gói cùng gói. Thay khi phát hành, nó là con số trong package.json.
const NPM_VERSION = require('../package.json').version;

/** Binary Rust đi kèm gói. */
const BINARY = process.env.CAMARELOL_BINARY || targetPath();

/** Cờ nào chỉ cần in phiên bản rồi thoát. */
function isVersionFlag(arg) {
  return arg === '-V' || arg === '--version' || arg === '--V' || arg === '-v';
}

/** In phiên bản của binary thật, không phải của gói. */
function printBinaryVersion() {
  if (!fs.existsSync(BINARY)) {
    say(`Không tìm thấy binary tại ${BINARY}`);
    say('Gói npm cài ra không có binary. Thử cài lại:');
    say('  npm i -g @wenker-ai/camarelo');
    process.exit(1);
  }
  const r = spawnSync(BINARY, ['--version'], { encoding: 'utf8' });
  process.stdout.write(r.stdout || '');
  process.stderr.write(r.stderr || '');
  process.exit(r.status === null ? 1 : r.status);
}

/**
 * Hỏi cờ cập nhật.
 *
 * Trả về true nghĩa là đã cập nhật xong và nên chạy tiếp bằng binary mới.
 */
async function offerUpdate() {
  let latest;
  try {
    latest = await latestRelease();
  } catch (_) {
    latest = null;
  }

  // Không có mạng: không hỏi gì cả. Hỏi rồi không trả lời được chỉ làm người
  // dùng tưởng phần mềm hỏng.
  if (!latest) return true;

  if (compare(NPM_VERSION, latest.version) >= 0) {
    // Bản trên npm không cũ hơn. Không nói gì — im lặng là tin tốt.
    return true;
  }

  say('');
  say('═══════════════════════════════════════════════════════════');
  say(` Bản trên npm có thể cũ: ${NPM_VERSION}, GitHub có ${latest.version}`);
  say('');
  say(' Cập nhật không? (y/n)');
  say('═══════════════════════════════════════════════════════════');

  const answer = await ask('> ');
  if (answer !== true) {
    // 'n' hoặc không ai trả lời — bỏ qua thông báo, đi tiếp.
    return true;
  }

  const ok = await selfUpdate(latest.version);
  return ok;
}

async function main() {
  const args = process.argv.slice(2);

  if (args.some(isVersionFlag)) {
    printBinaryVersion();
    return;
  }

  const updated = await offerUpdate();

  if (!updated) {
    // Cập nhật hỏng. Binary cũ vẫn chạy được, nên cứ chạy tiếp thay vì dừng.
    say('');
    say('Giữ nguyên bản cũ và tiếp tục.');
    say('');
  }

  // Chỉ dẫn Wi-Fi khi thật sự cần: không có mạng, và có giao diện không dây.
  if (!hasInternet() && interactive()) {
    await connectWifi();
  }

  if (!fs.existsSync(BINARY)) {
    say(`Không tìm thấy binary tại ${BINARY}`);
    process.exit(1);
  }

  // exec: thay tiến trình này bằng binary để tín hiệu, thư mục làm việc và mã
  // thoát đều đúng — không bọc thêm một lớp.
  const r = spawnSync(BINARY, args, { stdio: 'inherit' });
  process.exit(r.status === null ? 1 : r.status);
}

main().catch((err) => {
  say(`camarelol: ${err && err.message ? err.message : err}`);
  process.exit(1);
});
