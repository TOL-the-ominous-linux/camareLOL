'use strict';

/**
 * Hỏi có/không trên dòng lệnh.
 *
 * Trả về `null` khi không có ai trả lời — stdin không phải thiết bị đầu vào,
 * hoặc là kết thúc. Đó là trường hợp quan trọng: `npm install` chạy trong CI
 * không có người, và một câu hỏi chết đứng ở đó giết cả bài build.
 */

const readline = require('node:readline');

/** stdin có phải là terminal người dùng thật không. */
function interactive() {
  return Boolean(process.stdin.isTTY && process.stdout.isTTY);
}

function ask(question) {
  return new Promise((resolve) => {
    if (!interactive()) {
      resolve(null);
      return;
    }
    const rl = readline.createInterface({
      input: process.stdin,
      output: process.stdout,
    });
    let answered = false;
    rl.question(question, (answer) => {
      answered = true;
      const a = String(answer).trim().toLowerCase();
      // Phải quyết định *trước* khi đóng: `rl.close()` bắn sự kiện 'close'
      // ngay lập tức, và nó giải promise với null — promise chỉ nhận giá trị
      // đầu tiên, nên gọi close trước là mất câu trả lời của người dùng.
      if (a === 'y' || a === 'yes') resolve(true);
      else if (a === 'n' || a === 'no' || a === '') resolve(false);
      else resolve(null);
      rl.close();
    });
    rl.on('close', () => {
      if (!answered) resolve(null);
    });
  });
}

/** Hỏi một dòng, trả về chuỗi rỗng nếu không có ai trả lời. */
function askLine(question) {
  return new Promise((resolve) => {
    if (!interactive()) {
      resolve('');
      return;
    }
    const rl = readline.createInterface({
      input: process.stdin,
      output: process.stdout,
    });
    let answered = false;
    rl.question(question, (answer) => {
      answered = true;
      resolve(String(answer).trim());
      rl.close();
    });
    rl.on('close', () => {
      if (!answered) resolve('');
    });
  });
}

module.exports = { ask, askLine, interactive };
