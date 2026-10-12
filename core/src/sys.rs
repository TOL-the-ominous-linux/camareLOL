//! Safe wrapper around external tools.
//!
//! **Rule enforced by this module:** user-controlled data is *never* concatenated
//! into a shell string. Everything goes through [`std::process::Command`] with
//! separate arguments, so no value can break out into shell syntax.

use crate::error::{CamarelolError, ErrorCode, Result};
use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Result of running an external tool.
#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub program: String,
    pub args: Vec<String>,
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u128,
}

impl CommandOutput {
    pub fn success(&self) -> bool {
        self.status == 0
    }

    /// One-line description for logs. User input is NOT part of `args` because
    /// callers must only pass validated system paths; still, args are never
    /// re-parsed by a shell.
    pub fn describe(&self) -> String {
        format!("{} {} -> {}", self.program, self.args.join(" "), self.status)
    }
}

/// The only sanctioned way to run system tools from camareLOL.
pub fn run(program: &str, args: &[&str]) -> Result<CommandOutput> {
    run_with_timeout(program, args, Duration::from_secs(120))
}

/// Run a command with a timeout. On timeout the child is killed and
/// [`ErrorCode::CommandFailed`] is returned.
pub fn run_with_timeout(program: &str, args: &[&str], timeout: Duration) -> Result<CommandOutput> {
    let start = Instant::now();

    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Never inherit a secret-bearing environment: keep PATH, locale and home only.
    cmd.env_clear();
    for key in ["PATH", "HOME", "TERM", "LANG", "LC_ALL"] {
        if let Ok(val) = std::env::var(key) {
            cmd.env(key, val);
        }
    }

    let mut child = cmd.spawn().map_err(|e| {
        CamarelolError::new(
            ErrorCode::SpawnFailed,
            format!("could not start `{program}`"),
        )
        .with_context(e.to_string())
        .with_remediation("install the required package (util-linux / parted / arch-install-scripts)")
    })?;

    // Read pipes while waiting so that a chatty child cannot deadlock us.
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut so) = child.stdout.take() {
        let _ = so.read_to_string(&mut stdout);
    }
    if let Some(mut se) = child.stderr.take() {
        let _ = se.read_to_string(&mut stderr);
    }

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(CamarelolError::new(
                        ErrorCode::CommandFailed,
                        format!("`{program}` timed out after {}s", timeout.as_secs()),
                    ));
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => {
                return Err(CamarelolError::new(
                    ErrorCode::Internal,
                    format!("waiting for `{program}` failed: {e}"),
                ))
            }
        }
    };

    let code = status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
    Ok(CommandOutput {
        program: program.to_string(),
        args: args.iter().map(|s| s.to_string()).collect(),
        status: code,
        stdout,
        stderr,
        duration_ms: start.elapsed().as_millis(),
    })
}

/// Run a command feeding it `input` on stdin (no shell involved).
pub fn run_with_input(program: &str, args: &[&str], input: &str, timeout: Duration) -> Result<CommandOutput> {
    use std::io::Write;
    let start = Instant::now();
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.env_clear();
    for key in ["PATH", "HOME", "TERM", "LANG", "LC_ALL"] {
        if let Ok(val) = std::env::var(key) {
            cmd.env(key, val);
        }
    }
    let mut child = cmd.spawn().map_err(|e| {
        CamarelolError::new(ErrorCode::SpawnFailed, format!("could not start `{program}`"))
            .with_context(e.to_string())
    })?;
    if let Some(mut si) = child.stdin.take() {
        let _ = si.write_all(input.as_bytes());
    }
    let mut stdout = String::new();
    let mut stderr = String::new();
    if let Some(mut so) = child.stdout.take() {
        let _ = so.read_to_string(&mut stdout);
    }
    if let Some(mut se) = child.stderr.take() {
        let _ = se.read_to_string(&mut stderr);
    }
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(CamarelolError::new(
                        ErrorCode::CommandFailed,
                        format!("`{program}` timed out"),
                    ));
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => {
                return Err(CamarelolError::new(ErrorCode::Internal, e.to_string()))
            }
        }
    };
    Ok(CommandOutput {
        program: program.to_string(),
        args: args.iter().map(|s| s.to_string()).collect(),
        status: status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1),
        stdout,
        stderr,
        duration_ms: start.elapsed().as_millis(),
    })
}

/// True if `program` exists on PATH.
pub fn tool_available(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}

/// Read a whole file, returning a structured error instead of panicking.
pub fn read_file(path: &str) -> Result<String> {
    std::fs::read_to_string(path).map_err(|e| {
        CamarelolError::new(ErrorCode::ParseFailed, format!("cannot read {path}"))
            .with_context(e.to_string())
    })
}

/// True when `path` exists.
pub fn path_exists(path: &str) -> bool {
    std::path::Path::new(path).exists()
}
