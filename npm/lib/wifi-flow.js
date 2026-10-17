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
 * Mạng này có cần mật khẩu không?
 *
 * `iwctl station <dev> get-networks` in bảng có cột Security: `open` thì mở,
 * còn lại (`psk`, `wpa2`, `sae`…) thì cần mật khẩu. Quét trước rồi tra tên
 * mạng trong bảng; không thấy thì coi như không cần, để người dùng tự nhập
 * nếu họ muốn.
 */
function hasPassphrase(device, ssid) {
  iwctl(`station ${device} scan`);
  spawnSync('sh', ['-c', 'sleep 4']);
  const out = spawnSync(
    'sh',
    ['-c', `iwctl --command 'station ${device} get-networks' 2>/dev/null || true`],
    { encoding: 'utf8' },
  ).stdout;
  const security = parseSecurity(out, ssid);
  return security !== '' && security !== 'open';
}

/**
 * Cột Security của một mạng trong bảng `iwctl get-networks`.
 *
 * Tên mạng có thể chứa dấu cách ("My Home 5G"), nên không được tách theo khoảng
 * trắng rồi so phần tử đầu — phải khớp tiền tố rồi mới đọc phần còn lại.
 * Không thấy thì trả '' (không đoán).
 */
function parseSecurity(table, ssid) {
  for (const raw of table.split('\n')) {
    const line = raw.replace(/^\s+/, '');
    if (!line.startsWith(ssid)) continue;
    const rest = line.slice(ssid.length).trim();
    if (!rest) continue;
    const security = rest.split(/\s+/)[0].toLowerCase();
    // Dòng tiêu đề cột cũng bắt đầu bằng "Network name" — không phải mạng.
    if (/^(network|signal|security)$/i.test(security)) continue;
    return security;
  }
  return '';
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

    // Mật khẩu: hỏi TRƯỚC khi kết nối, vì `iwctl` sẽ dừng lại ở dấu nhắc
    // "Passphrase:" và chờ stdin. Mà ở đây ta gọi `iwctl --command`, tức mỗi
    // lệnh là một tiến trình riêng không có stdin là TTY — nên mạng có mã hoá
    // sẽ treo vĩnh viễn, và mọi mạng nhà đều có mã hoá. Đo được 2026-10-17
    // trên VirtualBox: `iwctl station wlan5 connect SecureNet` in
    // "Type the network passphrase for SecureNet psk." rồi đứng yên.
    // Mật khẩu đi trong argv của iwctl, nên trong `ps` của chính máy đó sẽ
    // thấy — đúng như mọi công cụ nối Wi-Fi khác, và còn hơn là ghi vào log.
    let pass = '';
    if (await hasPassphrase(device, answer)) {
      pass = await askLine('Mật khẩu (để trống nếu mạng mở): ');
      if (!pass) {
        say('Cần mật khẩu cho mạng này.');
        return false;
      }
    }

    // Mạng mở thì không thêm gì — `iwctl connect <ssid> ` với tham số rỗng
    // không phải cú pháp hợp lệ.
    const cmd = pass ? `station ${device} connect "${answer}" ${pass}` : `station ${device} connect "${answer}"`;
    iwctl(cmd);
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

module.exports = { connectWifi, iwctl, hasPassphrase };
// Chỉ để test: hàm tra thuần, không chạy iwctl.
module.exports.__test_parseSecurity = parseSecurity;
