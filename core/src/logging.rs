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

/// Words that mark the *name* of a secret rather than being one.
///
/// Compared against the name split on `-`, `_`, `.` and `:` so that `--api-key`
/// matches while `monkey` and `keyboard` do not.
const SENSITIVE_NAMES: &[&str] = &[
    "password",
    "passwd",
    "passphrase",
    "secret",
    "secrets",
    "token",
    "tokens",
    "key",
    "keys",
    "apikey",
    "apisecret",
    "credential",
    "credentials",
    "auth",
    "authorization",
    "bearer",
    // A Wi-Fi passphrase is a PSK: `wpa_passphrase=…`, `psk=…` and
    // `Psk=` all appear in real wpa_supplicant files and in the output of
    // anything that prints one. Three letters, never a word in an installer
    // log, and leaving it out meant `psk=hunter2` went into the log verbatim.
    "psk",
    // "luks" on its own is prose — "unlocking luks volume 2" — and appears in
    // most log lines this installer writes. The names that actually carry a
    // secret (luks_key, luks-passphrase) already match on "key" or
    // "passphrase".
];

/// Remove anything that could be a secret from a string before logging.
///
/// Best effort, and deliberately pessimistic: it would rather hide an innocent
/// word than leave a LUKS passphrase in a file.
///
/// The previous version looked for `name=value` inside a single whitespace-
/// separated word. That missed the two forms that actually occur in a command
/// line — `--api-key sk-abc` and `password: hunter2` — and redacted the *name*
/// while leaving the secret as the very next word, which is the one thing a
/// redaction pass must not do.
pub fn redact(input: &str) -> String {
    let words: Vec<(String, &str)> = input
        .split_inclusive(char::is_whitespace)
        .map(|w| {
            let t = w.trim_end();
            (t.to_string(), &w[t.len()..])
        })
        .collect();

    let mut out = String::with_capacity(input.len());
    let mut hiding = false;

    for (word, ws) in &words {
        if hiding {
            // Once a secret name has been seen the rest of the line goes. A
            // passphrase is several words long, and there is no way to tell
            // where it ends — so "correct horse battery" leaked two thirds of
            // itself under any rule that stopped at the first space.
            out.push_str("<redacted>");
            out.push_str(ws);
            continue;
        }

        let lower = word.to_ascii_lowercase();
        // Split "…" into name and inline value on the first = or :.
        let split_at = word.find(['=', ':']);
        let name = match split_at {
            Some(at) => &lower[..at],
            None => lower.as_str(),
        };

        if !is_sensitive_name(name) {
            out.push_str(word);
            out.push_str(ws);
            continue;
        }

        // Keep the name, drop everything from the separator onwards.
        match split_at {
            Some(at) => out.push_str(&word[..at]),
            None => out.push_str(word),
        }
        out.push_str("=<redacted>");
        out.push_str(ws);
        hiding = true;
    }
    out
}

/// True when any dot-separated segment of `name` is a known secret name.
fn is_sensitive_name(name: &str) -> bool {
    name.split(['-', '_', '.', ':', '/'])
        .any(|seg| SENSITIVE_NAMES.contains(&seg))
}

/// Redact a whole document, one line at a time.
///
/// [`redact`] is deliberately paranoid: once it sees a secret *name* it drops
/// everything to the end of the string it was given. That is right for a log
/// line, where the rest of the line really might be the rest of the
/// passphrase, and wrong for a report — it would eat the line breaks too, and a
/// diagnostics file whose headings have merged into one paragraph is a file
/// nobody can use.
///
/// So this is the *same* pass, applied per line, with the breaks put back.
/// There is deliberately no second, "weaker" matcher for documents: the point
/// is that a report a user attaches to a public bug tracker goes through the
/// exact code that guards the logs.
pub fn redact_document(input: &str) -> String {
    // `split` + `join` rather than a walk with an index: the split gives a line
    // without its newline, `redact` leaves the content alone when there is
    // nothing to hide, and joining puts the line breaks back. A trailing newline
    // in the input is preserved because `split` yields a final empty line.
    input
        .split('\n')
        .map(redact)
        .collect::<Vec<String>>()
        .join("\n")
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
        let file = create_dir_all(dir).ok().and_then(|_| {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .ok()
        });
        Self {
            file: Mutex::new(file),
            path,
        }
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

    /// The three that got through. Each one redacted the *name* and left the
    /// secret as the following word.
    #[test]
    fn a_flag_and_its_value_do_not_leak_the_value() {
        let out = redact("--api-key sk-abc123");
        assert!(!out.contains("sk-abc123"), "{out}");
    }

    #[test]
    fn a_colon_form_does_not_leak_the_value() {
        let out = redact("password: hunter2");
        assert!(!out.contains("hunter2"), "{out}");
    }

    #[test]
    fn an_authorization_header_is_gone_entirely() {
        // Assembled at run time rather than written out. A literal token in the
        // source is a live credential the moment someone copies it into a test,
        // and GitHub's push protection refuses the commit — which is the right
        // answer and a confusing thing to trip over for no reason.
        let token = format!("ghp_{}", "0".repeat(36));
        let out = redact(&format!("Authorization: Bearer {token}"));
        assert!(!out.contains(&token), "{out}");
    }

    #[test]
    fn a_luks_passphrase_does_not_leak() {
        let out = redact("unlocking luks-volume2 with luks_passphrase=correct horse battery");
        assert!(!out.contains("battery"), "{out}");
    }

    #[test]
    fn the_name_is_kept_so_the_line_still_reads() {
        let out = redact("user=alice password=hunter2");
        assert!(out.contains("password=<redacted>"), "{out}");
        assert!(out.contains("alice"), "{out}");
    }

    /// Over-redaction has its own cost: a log where every line is <redacted>
    /// cannot be used to work out what happened.
    #[test]
    fn ordinary_words_are_not_redacted() {
        for line in [
            "formatting /dev/nvme0n1p2",
            "keyboard layout detected",
            "monkey patching the bootloader",
            "found 2 partitions",
        ] {
            assert_eq!(redact(line), line, "{line}");
        }
    }

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

    // -------------------------------------------------- whole documents

    #[test]
    fn a_wifi_passphrase_does_not_leak() {
        // A Wi-Fi passphrase is a PSK and is written `psk=` in every wpa
        // configuration file. It was the one secret form with no name the
        // matcher recognised.
        let out = redact("ssid=HomeWiFi psk=correct horse battery staple");
        for part in ["correct", "horse", "battery", "staple"] {
            assert!(!out.contains(part), "`{part}` survived: {out}");
        }
        assert!(
            out.contains("ssid=HomeWiFi"),
            "the network name is not a secret: {out}"
        );
    }

    /// The document pass has to keep the line structure. `redact` eats the rest
    /// of the string it is given, so applying it to a whole file at once would
    /// fold every heading after the first secret into the same paragraph — and
    /// a report nobody can read is not a report.
    #[test]
    fn a_document_keeps_its_lines() {
        let input = "first line\npsk=hunter2\nthird line\nfourth line\n";
        let out = redact_document(input);
        assert_eq!(
            out.lines().count(),
            input.lines().count(),
            "no line may be lost or merged"
        );
        assert!(out.starts_with("first line\n"));
        assert!(out.ends_with("fourth line\n"));
        assert!(!out.contains("hunter2"), "{out}");
    }

    #[test]
    fn a_document_redacts_only_the_offending_lines() {
        let out = redact_document("cpu: 8 threads\npassword=hunter2\nram: 16 GiB\n");
        assert!(out.contains("cpu: 8 threads"), "{out}");
        assert!(out.contains("ram: 16 GiB"), "{out}");
        assert!(!out.contains("hunter2"), "{out}");
    }

    #[test]
    fn a_document_with_no_secrets_is_returned_unchanged() {
        for doc in [
            "",
            "one line, nothing private",
            "line one\nline two\nline three",
            "[Host]\ncpu: unknown\n",
        ] {
            assert_eq!(redact_document(doc), doc, "{doc:?}");
        }
    }
}
