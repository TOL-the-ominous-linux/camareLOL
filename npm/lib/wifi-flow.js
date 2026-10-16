'use strict';

/**
 * Quy trình nối Wi-Fi, chạy trước khi vào màn hình cài đặt.
 *
 * Thứ tự: xác định giao diện không dây → đưa ra danh sách → nạp → kết nối →
 * kiểm lại. Không tự động chọn mạng: chọn sai mạng nhà ngoại là cách tạo ra
 * một chiếc máy không có mạng mà người dùng không hiểu vì sao.
 */

const { spawnSync } = require('node:child_process');
const { askLine, interactive } = require('./prompt');
const { have, wirelessInterfaces, hasInternet, say } = require('./wifi');

/** Chạy một lệnh iwctl không cần giữ tiến trình. */
function iwctl(command) {
  return spawnSync('iwctl', ['--command', command], { encoding: 'utf8' });
}

/**
 * Nối vào một mạng.
 *
 * @returns {boolean} true nếu có mạng sau khi thử.
 */
async function connectWifi() {
  if (!have('iwctl')) {
    say('');
    say('Không tìm thấy iwctl — không nối được Wi-Fi.');
    say('Cài nó bằng:  sudo pacman -S iwd   rồi chạy lại.');
    say('');
    return false;
  }

  if (hasInternet()) {
    say('Đã có mạng. Không cần nối Wi-Fi.');
    return true;
  }

  const ifaces = wirelessInterfaces();
  if (ifaces.length === 0) {
    say('');
    say('Không thấy giao diện không dây nào (wlan0…).');
    say('Nếu máy có Wi-Fi nhưng không thấy, có thể chưa bật firmware hoặc phần cứng.');
    say('');
    return false;
  }

  say('');
  say('=== Nối Wi-Fi ===');
  say(`Giao diện không dây: ${ifaces.join(', ')}`);
  say('');

  if (!interactive()) {
    say('Không có ai để hỏi (đang chạy không tương tác).');
    say('Nối Wi-Fi bằng tay rồi chạy lại camarelol.');
    return false;
  }

  // Cho họ chạy nhiều lần: mạng đúng thường là lần thứ hai, khi đã nhớ mật khẩu.
  for (let attempt = 1; attempt <= 3; attempt++) {
    say(`--- lần ${attempt} ---`);
    iwctl('device list');
    const device = (await askLine('Tên giao diện (Enter = dùng giao diện đầu tiên): '))
      || ifaces[0];

    say('');
    say('Các mạng đã lưu:');
    const saved = spawnSync('sh', ['-c', 'netctl list 2>/dev/null | grep "^\\*" || true'], {
      encoding: 'utf8',
    }).stdout
      .split('\n')
      .map((s) => s.replace(/^\*\s*/, '').trim())
      .filter(Boolean);

    if (saved.length) saved.forEach((n, i) => say(`  ${i + 1}) ${n}`));
    else say('  (chưa lưu mạng nào)');
    say('');

    const answer = await askLine('Tên mạng (SSID): ');
    if (!answer) {
      say('Bỏ qua nối Wi-Fi.');
      return false;
    }

    iwctl(`station ${device} connect "${answer}"`);
    // Cho iwd thời gian xác nhập và lấy địa chỉ.
    spawnSync('sh', ['-c', 'sleep 8']);

    if (hasInternet()) {
      say('');
      say(`Đã nối "${answer}" và có mạng.`);
      return true;
    }
    say('Chưa có mạng. Thử tên khác, hoặc kiểm tra mật khẩu.');
    say('');
  }

  say('Không nối được sau 3 lần thử.');
  return false;
}

module.exports = { connectWifi, iwctl };
