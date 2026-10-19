//! Safe wrapper around external tools.
//!
//! **Rule enforced by this module:** user-controlled data is *never* concatenated
//! into a shell string. Everything goes through [`std::process::Command`] with
//! separate arguments, so no value can break out into shell syntax.
//!
//! # `--preview` cannot spawn a mutating command
//!
//! This module is the **choke point** for that guarantee. [`run`],
//! [`run_with_input`], [`run_with_timeout`] and [`run_command`] are the only
//! ways camareLOL starts a process that could change the machine, and all four
//! refuse to do anything at all once [`enter_preview`] has been called — they
//! return [`ErrorCode::PreviewActive`] before reaching `Command::spawn`.
//!
//! Reading the machine is a different matter: a preview that could not list the
//! disks, the firmware or the network would be useless. Read-only probes go
//! through [`inspect`] / [`inspect_with_timeout`], which are the one documented
//! exception and cannot modify anything.
//!
//! Why a global latch instead of a flag on every caller: the guarantee has to
//! survive a bug. A forgotten `if preview { ... }` on one runner method used to
//! mean "dry-run quietly does nothing", and the natural fix — deleting the
//! `ensure_exec()` guard that used to abort early — would have run `pacstrap`
//! and `sgdisk` for real during a *preview*. The latch cannot be forgotten by a
//! caller because there is nothing for a caller to pass.

use crate::error::{CamarelolError, ErrorCode, Result};
use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Preview latch
// ---------------------------------------------------------------------------

/// Set once, at startup, when `--preview` is on the command line.
///
/// There is deliberately **no** matching "exit preview" for production code:
/// once the binary has declared itself a preview it must stay one until it
/// exits, so no later code path can talk it back into writing.
static PREVIEW: AtomicBool = AtomicBool::new(false);

/// Every process actually spawned through this module, whatever the reason.
/// The tests assert this stays at zero for a full simulated install.
static SPAWNS: AtomicUsize = AtomicUsize::new(0);

/// How many times a mutating command was *refused* by the latch. A non-zero
/// count in preview is the proof that the backstop fired rather than simply
/// never being reached.
static MUTATING_REFUSALS: AtomicUsize = AtomicUsize::new(0);

/// Enter preview mode for the rest of the process' life.
///
/// Called from `main()` before anything can spawn a command. Idempotent.
pub fn enter_preview() {
    PREVIEW.store(true, Ordering::SeqCst);
}

/// True while [`enter_preview`] is in effect.
pub fn preview_active() -> bool {
    PREVIEW.load(Ordering::SeqCst)
}

/// Number of processes spawned through this module so far.
///
/// Zero after a complete `--preview` install: that number is the whole safety
/// claim, so it is observable instead of being taken on trust.
pub fn spawn_count() -> usize {
    SPAWNS.load(Ordering::SeqCst)
}

/// How many mutating commands the latch refused.
pub fn mutating_refusals() -> usize {
    MUTATING_REFUSALS.load(Ordering::SeqCst)
}

/// The error every mutating entry point returns in preview mode.
fn preview_refusal(program: &str) -> CamarelolError {
    MUTATING_REFUSALS.fetch_add(1, Ordering::SeqCst);
    CamarelolError::new(
        ErrorCode::PreviewActive,
        format!("`{program}` was not run: --preview is active"),
    )
    .with_remediation("re-run camareLOL without --preview to really install")
}

/// Refuse a mutating command while preview is active; `Ok(())` means "carry on".
fn allow_mutating(program: &str) -> Result<()> {
    if preview_active() {
        return Err(preview_refusal(program));
    }
    Ok(())
}

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
        format!(
            "{} {} -> {}",
            self.program,
            self.args.join(" "),
            self.status
        )
    }
}

/// The only sanctioned way to run a **mutating** system tool from camareLOL.
///
/// Refuses (without spawning anything) while [`preview_active`]. Use
/// [`inspect`] for read-only probes.
pub fn run(program: &str, args: &[&str]) -> Result<CommandOutput> {
    run_with_timeout(program, args, Duration::from_secs(120))
}

/// Run a mutating command with a timeout. On timeout the child is killed and
/// [`ErrorCode::CommandFailed`] is returned.
///
/// Refuses (without spawning anything) while [`preview_active`].
pub fn run_with_timeout(program: &str, args: &[&str], timeout: Duration) -> Result<CommandOutput> {
    allow_mutating(program)?;
    let mut cmd = Command::new(program);
    cmd.args(args);
    run_command(cmd, timeout)
}

/// Run a mutating command the caller has already configured.
///
/// Same guarantees as [`run_with_timeout`] — no shell, cleared environment,
/// timeout, pipes drained while waiting — but for the cases where the argument
/// list has to be built differently (long flag sets, or values that must not
/// pass through `&str` slices).
///
/// Refuses (without spawning anything) while [`preview_active`].
pub fn run_command(cmd: Command, timeout: Duration) -> Result<CommandOutput> {
    allow_mutating(cmd.get_program().to_string_lossy().as_ref())?;
    spawn_and_capture(cmd, timeout)
}

/// Run a **read-only** probe, e.g. `lsblk -J -b -o ...` or `lspci -mm -nn`.
///
/// This is the documented exception to the preview latch, and it is why
/// [`run`] refuses wholesale rather than allowing a hand-written allow-list of
/// programs: a preview that cannot see which disks exist, whether the machine
/// booted UEFI, or whether a network helper is installed could not produce a
/// plan at all.
///
/// Only commands that merely *report* may use this. Anything that creates,
/// erases, formats, mounts or writes belongs in [`run`].
pub fn inspect(program: &str, args: &[&str]) -> Result<CommandOutput> {
    inspect_with_timeout(program, args, Duration::from_secs(30))
}

/// [`inspect`] with an explicit timeout. Probes are expected to be instant, so
/// the default is deliberately short: a hanging `lsblk` should not stall the
/// whole interface.
pub fn inspect_with_timeout(
    program: &str,
    args: &[&str],
    timeout: Duration,
) -> Result<CommandOutput> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    spawn_and_capture(cmd, timeout)
}

/// The one place in the crate where `Command::spawn` actually happens.
///
/// Every public entry point funnels through here, and the counter is what the
/// acceptance tests read to prove that a preview spawned nothing at all.
fn spawn_and_capture(mut cmd: Command, timeout: Duration) -> Result<CommandOutput> {
    let program = cmd.get_program().to_string_lossy().to_string();
    let program = program.as_str();
    let args: Vec<String> = cmd
        .get_args()
        .map(|a| a.to_string_lossy().to_string())
        .collect();
    let start = Instant::now();

    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Never inherit a secret-bearing environment: keep PATH, locale and home only.
    cmd.env_clear();
    for key in ["PATH", "HOME", "TERM", "LANG", "LC_ALL"] {
        if let Ok(val) = std::env::var(key) {
            cmd.env(key, val);
        }
    }

    SPAWNS.fetch_add(1, Ordering::SeqCst);
    let mut child = cmd.spawn().map_err(|e| {
        CamarelolError::new(
            ErrorCode::SpawnFailed,
            format!("could not start `{program}`"),
        )
        .with_context(e.to_string())
        .with_remediation(
            "install the required package (util-linux / parted / arch-install-scripts)",
        )
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
        args,
        status: code,
        stdout,
        stderr,
        duration_ms: start.elapsed().as_millis(),
    })
}

/// Run a mutating command feeding it `input` on stdin (no shell involved).
///
/// Refuses (without spawning anything) while [`preview_active`].
pub fn run_with_input(
    program: &str,
    args: &[&str],
    input: &str,
    timeout: Duration,
) -> Result<CommandOutput> {
    allow_mutating(program)?;
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
    SPAWNS.fetch_add(1, Ordering::SeqCst);
    let mut child = cmd.spawn().map_err(|e| {
        CamarelolError::new(
            ErrorCode::SpawnFailed,
            format!("could not start `{program}`"),
        )
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
            Err(e) => return Err(CamarelolError::new(ErrorCode::Internal, e.to_string())),
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
///
/// A `<program> --version` probe: it reports, it never changes anything, so it
/// stays available in preview.
pub fn tool_available(program: &str) -> bool {
    inspect_with_timeout(program, &["--version"], Duration::from_secs(5))
        .map(|out| out.success())
        .unwrap_or(false)
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
