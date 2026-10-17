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

/**
 * Có mạng chưa.
 *
 * Một lần thử trước đây hỏng vì lý do rất thật: đo trên máy thật ngày
 * 2026-10-17, lần gọi đầu tiên trả `false` trong khi máy đang có mạng — DNS và
 * TLS lạnh vượt quá `--max-time 5`. Hậu quả là hỏi người dùng "kết nối
 * Wi-Fi?" đúng lúc họ đã có mạng.
 *
 * Vì vậy: thử nhiều đích, và coi **bất kỳ mã HTTP nào** là có mạng. 4xx/5xx
 * vẫn chứng minh máy chạm được một máy chủ khác — chỉ "không chạm được" mới là
 * không có mạng. `-I` dùng HEAD nên nhẹ hơn GET một trang HTML.
 */
const NET_PROBES = [
  'https://archlinux.org',
  'https://mirror.rackspace.com',
  'https://registry.npmjs.org',
];

// `probe` nhận một url và trả mã thoát của curl. Mặc định là curl thật; test
// thay bằng hàm giả để kiểm logic thử nhiều đích mà không chạm mạng.
function hasInternet(probe) {
  const run = probe || defaultProbe;
  for (const url of NET_PROBES) {
    if (run(url) === 0) return true;
  }
  return false;
}

function defaultProbe(url) {
  return spawnSync(
    'curl',
    ['-sS', '-I', '--max-time', '8', '-o', '/dev/null', url],
    { encoding: 'utf8' },
  ).status;
}

module.exports = { have, wirelessInterfaces, savedNetworks, hasInternet, say };
