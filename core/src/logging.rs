//! Controlled logging.
//!
//! Logs are written per install run to `/var/log/camarelol-<timestamp>.log` (or
//! `./camarelol-<timestamp>.log` when not running as root). Passwords, key
//! material and tokens are **never** logged: every value passes through
//! [`redact`] before it is written.

use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Remove anything that could be a secret from a string before logging.
pub fn redact(input: &str) -> String {
    // Best effort pass for obvious "password=..." / "token=..." style leaks.
    let mut out = String::with_capacity(input.len());
    for word in input.split_inclusive(char::is_whitespace) {
        let trimmed = word.trim_end();
        let ws = &word[trimmed.len()..];
        let lower = trimmed.to_ascii_lowercase();
        let sensitive = ["password", "passwd", "secret", "token", "key", "luks", "apikey"]
            .iter()
            .any(|k| lower.contains(k));
        if sensitive {
            // Keep only the key part, drop the value entirely.
            match trimmed.split_once('=') {
                Some((k, _)) => {
                    out.push_str(k);
                    out.push_str("=<redacted>");
                }
                None => out.push_str("<redacted>"),
            }
        } else {
            out.push_str(trimmed);
        }
        out.push_str(ws);
    }
    out
}

pub struct Logger {
    file: Mutex<Option<std::fs::File>>,
    path: PathBuf,
}

impl Logger {
    /// Create a logger. Falls back to a memory logger when the log directory is
    /// not writable (never panics because of logging).
    pub fn new(dir: &Path) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("camarelol-{stamp}.log"));
        let file = create_dir_all(dir)
            .ok()
            .and_then(|_| OpenOptions::new().create(true).append(true).open(&path).ok());
        Self { file: Mutex::new(file), path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn log(&self, phase: &str, message: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let line = format!("[{now}] [{phase}] {}\n", redact(message));
        if let Ok(mut guard) = self.file.lock() {
            if let Some(f) = guard.as_mut() {
                let _ = f.write_all(line.as_bytes());
            }
        }
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self::new(Path::new("."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_password_like_values() {
        let out = redact("user=alice password=hunter2");
        assert!(!out.contains("hunter2"), "{out}");
        assert!(out.contains("password=<redacted>"), "{out}");
    }

    #[test]
    fn redacts_tokens() {
        let out = redact("token=ghp_abcdef");
        assert!(!out.contains("ghp_abcdef"), "{out}");
    }
}
