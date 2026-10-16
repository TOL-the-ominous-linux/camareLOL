'use strict';

/**
 * Kết nối Wi-Fi bằng iwctl.
 *
 * camareLOL cần mạng để tải gói Arch. Trên một máy cài mới, Wi-Fi là thứ
 * thường chưa nối — và `iwctl` là công cụ mà Arch live ISO có sẵn.
 *
 * Vì sao gọi `iwctl` như một tiến trình con thay vì nhúng thẳng: iwctl là REPL,
 * nó đọc từ stdin và ghi ra stdout. Nói chuyện với nó từ Node cần đồng bộ từng
 * bước chờ dấu nhắc. Cách dễ hơn một chút: sinh tệp cấu hình rồi chạy
 * `iwctl --command <lệnh>` cho từng lệnh, không cần giữ tiến trình sống.
 */

const { spawnSync } = require('node:child_process');

function say(line) {
  process.stdout.write(line + '\n');
}

function have(cmd) {
  return spawnSync('sh', ['-c', `command -v ${cmd}`], { encoding: 'utf8' }).status === 0;
}

/** Các giao diện không dây, theo đúng thứ tự người dùng sẽ thấy. */
function wirelessInterfaces() {
  const r = spawnSync('sh', ['-c', "ls /sys/class/net 2>/dev/null"], { encoding: 'utf8' });
  if (r.status !== 0) return [];
  const out = [];
  for (const name of r.stdout.split('\n').map((s) => s.trim()).filter(Boolean)) {
    // `wireless` là giao diện wlan; `phy80211` là phy (wlan0, phy0...).
    const hasWireless = require('node:fs').existsSync(`/sys/class/net/${name}/wireless`);
    const isPhy = name.startsWith('phy');
    if (hasWireless && !isPhy) out.push(name);
  }
  return out;
}

/** Mạng đã lưu, nếu iwctl có. */
function savedNetworks() {
  if (!have('iwctl')) return [];
  const r = spawnSync('sh', ['-c', 'iwctl station wlan0 list 2>/dev/null || true'], {
    encoding: 'utf8',
  });
  void r;
  // iwctl không liệt kê mạng đã lưu qua CLI một cách ổn định; ta dựa vào
  // `netctl list` khi có.
  const n = spawnSync('sh', ['-c', 'netctl list 2>/dev/null | grep "^\\*" || true'], {
    encoding: 'utf8',
  });
  return n.stdout
    .split('\n')
    .map((s) => s.replace(/^\*\s*/, '').trim())
    .filter(Boolean);
}

/** Có mạng chưa. */
function hasInternet() {
  const r = spawnSync('sh', ['-c', 'curl -sS --max-time 5 -o /dev/null https://archlinux.org 2>/dev/null'],
    { encoding: 'utf8' });
  return r.status === 0;
}

module.exports = { have, wirelessInterfaces, savedNetworks, hasInternet, say };
