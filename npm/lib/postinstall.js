#!/usr/bin/env node
'use strict';

/**
 * Chạy sau `npm install`.
 *
 * Nguyên tắc của npm: `postinstall` chạy ở ngữ cảnh không tương tác — CI, Docker
 * build, `npm ci`. Một prompt chết đứng ở đây giết cả bài build, nên tệp này
 * **không hỏi gì**. Nó chỉ kiểm tra xem máy có dùng được không và nói ra điều
 * cần làm.
 *
 * Việc hỏi/cho chọn xảy ra khi chạy `camarelol`, ở lib/ và bin/.
 */

const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const { have, wirelessInterfaces } = require('./wifi');

const problems = [];
const notes = [];

function ok(line) { notes.push(`  ok    ${line}`); }
function bad(line) { problems.push(`  thiếu ${line}`); }

function main() {
  // 1. Hệ điều hành.
  let osName = 'không rõ';
  try {
    const rel = fs.readFileSync('/etc/os-release', 'utf8');
    const id = rel.match(/^ID=("?)(.+?)\1$/m);
    osName = id ? id[2] : 'không rõ';
  } catch (_) {
    /* không đọc được thì để vậy */
  }

  if (osName === 'arch' || osName === 'archarm') {
    ok(`hệ điều hành: ${osName}`);
  } else {
    // Không phải lỗi cứng: chạy được, chỉ là ngoài dự kiến.
    notes.push(`  lưu ý  hệ điều hành là "${osName}", không phải Arch.`);
    notes.push('         camareLOL viết cho Arch Linux; nó có thể không đúng trên đây.');
  }

  // 2. Kiến trúc — gói này chỉ đóng gói binary x64.
  if (process.arch !== 'x64') {
    bad(`kiến trúc ${process.arch}; gói này chỉ có binary x64.`);
    notes.push('         Nếu bạn dùng arm64, hãy cài trực tiếp từ GitHub Releases.');
  } else {
    ok('kiến trúc: x64');
  }

  // 3. Công cụ mà quy trình cài cần.
  const need = [
    ['curl', 'tải gói Arch'],
    ['pacman', 'cài hệ thống'],
    ['git', 'theo dõi camareLOL'],
    ['sudo', 'chạy lệnh với quyền hệ thống'],
  ];
  for (const [cmd, why] of need) {
    if (have(cmd)) ok(`${cmd} — ${why}`);
    else bad(`${cmd} — ${why}`);
  }

  // 4. Wi-Fi chỉ là thông tin, không phải yêu cầu.
  if (have('iwctl')) {
    const ifaces = wirelessInterfaces();
    if (ifaces.length) ok(`iwctl và ${ifaces.join(', ')} — sẽ hỏi nếu chưa có mạng`);
    else ok('iwctl có, chưa thấy giao diện không dây');
  } else {
    notes.push('  lưu ý  không có iwctl — nếu máy chỉ có Wi-Fi, cài: sudo pacman -S iwd');
  }

  // 5. Binary đi kèm.
  const bin = path.join(__dirname, '..', 'vendor', 'camarelol');
  if (fs.existsSync(bin)) {
    const r = spawnSync(bin, ['--version'], { encoding: 'utf8' });
    const first = (r.stdout || '').split('\n')[0] || '(không phản hồi)';
    ok(`binary: ${first.trim()}`);
  } else {
    bad(`binary không có tại ${bin}`);
  }

  process.stdout.write('\ncamareLOL — kiểm tra sau khi cài\n\n');
  for (const n of notes) process.stdout.write(n + '\n');
  process.stdout.write('\n');

  if (problems.length) {
    process.stdout.write('Có thứ còn thiếu:\n');
    for (const p of problems) process.stdout.write(p + '\n');
    process.stdout.write('\nTrên Arch, cài chúng bằng:\n');
    process.stdout.write('  sudo pacman -S curl git sudo iwd\n\n');
    // Không exit(1): gói đã cài xong, và việc thiếu `curl` không làm hỏng nó.
    // Thoát số khác 0 sẽ làm `npm install` báo lỗi và nhiều người sẽ hiểu là
    // cài thất bại.
  }

  process.stdout.write('Chạy `camarelol` để bắt đầu.\n');
  process.stdout.write('Chạy `camarelol --version` để xem phiên bản.\n\n');
  process.exit(0);
}

try {
  // `require` tệp này trong một bài kiểm tra không được chạy phần kiểm tra:
  // require phải có tác dụng phụ, không phải in ra báo cáo.
  if (require.main === module) {
    main();
  }
} catch (err) {
  process.stdout.write(`camareLOL: kiểm tra sau cài lỗi: ${err && err.message}\n`);
  process.exit(0);
}
