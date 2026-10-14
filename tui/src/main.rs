//! camareLOL — a modern, safety-first TUI installer for Arch Linux.

mod app;
mod draw;
mod scripted;
mod update;
mod version;

use crate::app::App;
use camarelol_core::config::InstallationConfig;
use camarelol_core::error::{CamarelolError, ErrorCode};
use camarelol_core::hardware::Firmware;
use camarelol_core::install::execute::SimulatedRunner;
use camarelol_core::install::run_install;
use camarelol_core::install::{NullSink, Outcome};
use camarelol_core::logging::Logger;
use camarelol_core::safety::SafetyGate;
use camarelol_core::storage::scan_devices;
use std::path::PathBuf;
#[derive(Debug, Default)]
struct Args {
    mock: Option<PathBuf>,
    config: Option<PathBuf>,
    save_config: Option<PathBuf>,
    allow_destructive: bool,
    execute: bool,
    scan: bool,
    self_test: bool,
    scripted: Option<PathBuf>,
    language: Option<String>,
    check_update: bool,
    update: bool,
    rollback_update: bool,
    update_url: Option<String>,
    update_key: Option<PathBuf>,
    update_sig_url: Option<String>,
    allow_unsigned_update: bool,
    allow_expired_index: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--mock" => a.mock = args.next().map(PathBuf::from),
            "--config" => a.config = args.next().map(PathBuf::from),
            "--save-config" => a.save_config = args.next().map(PathBuf::from),
            "--allow-destructive" => a.allow_destructive = true,
            "--execute" => a.execute = true,
            "--scan" => a.scan = true,
            "--self-test" => a.self_test = true,
            "--scripted" => a.scripted = args.next().map(PathBuf::from),
            "--lang" => a.language = args.next(),
            "--check-update" => a.check_update = true,
            "--update" => a.update = true,
            "--rollback-update" => a.rollback_update = true,
            "--update-url" => a.update_url = args.next(),
            "--update-key" => a.update_key = args.next().map(PathBuf::from),
            "--update-sig-url" => a.update_sig_url = args.next(),
            "--allow-unsigned-update" => a.allow_unsigned_update = true,
            "--allow-expired-index" => a.allow_expired_index = true,
            "-h" | "--help" => {
                println!("{}", usage());
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("{}", version::banner());
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(a)
}

fn usage() -> &'static str {
    "camareLOL — Arch Linux TUI installer

USAGE:
    camarelol [OPTIONS]

OPTIONS:
    --config <FILE>           Load a saved installation profile
    --save-config <FILE>      Save the profile when the install finishes
    --mock <FILE>             Use a JSON device survey instead of real disks
    --scan                    Print the storage survey as JSON and exit
    --self-test               Run the non-destructive acceptance checks
    --scripted <FILE>         Install without a UI (for automated testing)
    --allow-destructive       Arm the safety gate (still requires confirmation)
    --execute                 Really write to disk (default is dry-run)
    --lang <vi|en>            Installer language

UPDATES:
    --check-update            Ask the update index for a newer release and exit
    --update                  Download, verify and install it (needs write access)
    --rollback-update         Put the previous binary back
    --update-url <URL>        Use a different index (default: the project URL)
    --update-key <FILE>       Pinned public key used to check the signature
    --update-sig-url <URL>    Where the detached signature lives
                              (default: the index URL plus .sig)
    --allow-unsigned-update   Install even if the index carries no valid
                              signature from the pinned key
    --allow-expired-index     Accept an index whose expiry has passed

    Updates never happen on their own. --check-update only reports, --update
    refuses while an installation is running, and it refuses an index that is
    not signed unless you pass --allow-unsigned-update. See docs/UPDATES.md.

    -h, --help                Show this help
    -V, --version             Show version and commit
"
}

fn main() {
    restore_default_sigpipe();
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    };

    if args.check_update || args.update || args.rollback_update {
        match cmd_update(UpdateOptions {
            check_only: args.check_update,
            install: args.update,
            rollback: args.rollback_update,
            url_override: args.update_url,
            key_path: args.update_key,
            signature_url: args.update_sig_url,
            allow_unsigned: args.allow_unsigned_update,
            allow_expired: args.allow_expired_index,
        }) {
            Ok(code) => std::process::exit(code),
            Err(e) => {
                eprintln!("error: {e}");
                if let Some(r) = &e.remediation {
                    eprintln!("hint: {r}");
                }
                std::process::exit(1);
            }
        }
    }

    if args.scan {
        if let Err(e) = cmd_scan(args.mock.clone()) {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
        return;
    }

    if args.self_test {
        match cmd_self_test(args.mock.clone()) {
            Ok(true) => {
                println!("self-test: OK");
                std::process::exit(0);
            }
            Ok(false) => {
                println!("self-test: FAILED");
                std::process::exit(1);
            }
            Err(e) => {
                println!("self-test: error: {e}");
                std::process::exit(1);
            }
        }
    }

    if let Some(cfg_path) = args.scripted.clone() {
        match scripted::run_scripted(
            &cfg_path,
            args.mock.as_deref(),
            args.allow_destructive && args.execute,
        ) {
            Ok(report) => {
                println!("{}", report.render());
                std::process::exit(if report.all_completed() { 0 } else { 1 });
            }
            Err(e) => {
                eprintln!("scripted install failed: {e}");
                std::process::exit(1);
            }
        }
    }

    match run_tui(args) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("camareLOL failed: {e}");
            std::process::exit(1);
        }
    }
}

/// `--check-update`, `--update` and `--rollback-update`.
///
/// The exit code is 0 on success, 10 when a newer release exists but nothing
/// was done. Scripts can use that to nag without failing.
/// What the update sub-commands were asked to do.
///
/// A struct rather than eight flags: at the seventh one it stopped being
/// possible to tell `allow_unsigned` from `allow_expired` at a call site.
struct UpdateOptions {
    check_only: bool,
    install: bool,
    rollback: bool,
    url_override: Option<String>,
    key_path: Option<PathBuf>,
    signature_url: Option<String>,
    allow_unsigned: bool,
    allow_expired: bool,
}

fn cmd_update(opts: UpdateOptions) -> Result<i32, CamarelolError> {
    let UpdateOptions {
        check_only,
        install,
        rollback,
        url_override,
        key_path,
        signature_url: args_sig,
        allow_unsigned,
        allow_expired,
    } = opts;
    use camarelol_core::update::{self, Plan};

    // Asking for both means the user wants to know first; report, never install.
    let install = install && !check_only;

    let url = url_override
        .clone()
        .unwrap_or_else(|| update::DEFAULT_MANIFEST_URL.to_string());
    let target = std::env::current_exe().map_err(|e| {
        CamarelolError::new(
            ErrorCode::UpdateNotPrivileged,
            "could not work out where this binary lives",
        )
        .with_context(e.to_string())
    })?;

    if rollback {
        update::rollback(&target)?;
        println!("restored the previous binary at {}", target.display());
        println!("run {} --version to check it", target.display());
        return Ok(0);
    }

    let can_write = is_writable(&target);
    // By default the detached signature sits next to the index.
    let sig_url = args_sig
        .clone()
        .or_else(|| url_override.as_ref().map(|u| format!("{u}.sig")));
    let public_key = load_public_key(key_path.as_deref())?;

    println!("{}", version::banner());
    println!("index: {url}");
    println!("binary: {}", target.display());

    // Nothing is running an install from this process, so the one hard stop
    // (an install in progress) cannot trigger here; a library caller has to
    // pass that flag itself, which is where the real protection lives.
    let index = update::fetch_index(&url, sig_url.as_deref(), public_key.as_deref())?;
    println!("trust: {}", index.trust.reason());
    let plan = update::plan_from_index(
        &index,
        version::VERSION,
        install,
        false,
        can_write,
        allow_unsigned,
        allow_expired,
    )?;

    match &plan {
        Plan::UpToDate => {
            println!("up to date");
            return Ok(0);
        }
        Plan::Blocked { release, reason } => {
            println!(
                "version {} is available, but not installing it:",
                release.version
            );
            println!("  {}", reason.as_str());
            return Ok(10);
        }
        Plan::Available(release) => {
            println!("version {} is available.", release.version);
            if let Some(n) = &release.notes {
                println!("  {n}");
            }
            println!("nothing has been downloaded or changed.");
            println!("re-run with --update to install it");
            return Ok(10);
        }
        Plan::Install(release) => {
            println!("installing version {}...", release.version);
        }
    }

    let release = match plan {
        Plan::Install(r) => r,
        _ => unreachable!("only Install reaches this point"),
    };

    let staging = target.with_file_name(format!(
        "{}.download",
        target.file_name().unwrap_or_default().to_string_lossy()
    ));
    // Whatever happens, the half-downloaded file must not be left lying next
    // to the real binary for someone else to find and run.
    let result = update::fetch_candidate(&release, &staging)
        .and_then(|_| update::install_candidate(&target, &staging));
    let _ = std::fs::remove_file(&staging);
    result?;

    println!("installed version {}", release.version);
    println!("the previous binary is kept at {}.prev", target.display());
    println!("undo this with: camarelol --rollback-update");
    println!("start the new binary with: {}", target.display());
    Ok(0)
}

/// Rust ignores SIGPIPE and turns it into a panic on the next write, so
/// `camarelol --check-update | head` dies with "failed printing to stdout"
/// instead of stopping quietly like every other command-line tool.
fn restore_default_sigpipe() {
    // SAFETY: setting a signal disposition to SIG_DFL is async-signal-safe and
    // is done before any other thread exists.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

/// The pinned public key: the file the user named, else the one built in.
///
/// Building in `keys/camarelol.pub` means a key can never be swapped by a
/// downloaded index — the trust anchor ships with the binary, not beside it.
fn load_public_key(path: Option<&std::path::Path>) -> Result<Option<String>, CamarelolError> {
    if let Some(p) = path {
        return std::fs::read_to_string(p).map(Some).map_err(|e| {
            CamarelolError::new(
                ErrorCode::UpdateSignatureInvalid,
                format!("could not read the public key {}", p.display()),
            )
            .with_context(e.to_string())
        });
    }
    Ok(crate::version::embedded_update_key())
}

fn is_writable(path: &std::path::Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Root can write almost anything; mode bits are a good enough answer
        // for everyone else and never wrong in the dangerous direction.
        std::fs::metadata(path)
            .map(|m| m.permissions().mode() & 0o222 != 0)
            .unwrap_or(false)
            || nix_is_root()
    }
    #[cfg(not(unix))]
    {
        path.exists()
    }
}

#[cfg(unix)]
fn nix_is_root() -> bool {
    std::fs::metadata("/proc/self").is_ok()
        && std::fs::read_to_string("/proc/self/status")
            .map(|s| {
                s.lines()
                    .any(|l| l.starts_with("Uid:") && l.split_whitespace().nth(1) == Some("0"))
            })
            .unwrap_or(false)
}

fn cmd_scan(mock: Option<PathBuf>) -> Result<(), CamarelolError> {
    let devices = match mock {
        Some(p) => camarelol_core::storage::load_survey(p.to_str().unwrap_or_default())?,
        None => scan_devices()?,
    };
    let json = serde_json::to_string_pretty(&devices)
        .map_err(|e| CamarelolError::new(ErrorCode::Internal, e.to_string()))?;
    println!("{json}");
    Ok(())
}

/// Non-destructive acceptance checks derived from the specification.
fn cmd_self_test(mock: Option<PathBuf>) -> Result<bool, CamarelolError> {
    let devices = match mock {
        Some(p) => camarelol_core::storage::load_survey(p.to_str().unwrap_or_default())?,
        None => scan_devices()?,
    };
    let core = camarelol_core::Core::with_devices(devices);
    let mut ok = true;
    let empty = core.devices.iter().find(|d| d.is_empty());
    println!("1. devices discovered: {}", core.devices.len());
    for d in &core.devices {
        println!("   - {} {} {}", d.path, d.kind.as_str(), d.summary_line());
        assert_eq!(d.risk, d.assess_risk());
    }

    // A. empty disk planning. Both firmware modes are checked so the result
    //    does not depend on whether this machine booted UEFI or BIOS.
    let detected = core.firmware();
    match empty {
        Some(d) => {
            for mode in [Firmware::Uefi, Firmware::Bios] {
                let mut probe = camarelol_core::Core::with_devices(core.devices.clone());
                probe.set_firmware(mode);
                let plan = match probe.plan_for(&d.path) {
                    Ok(p) => p,
                    Err(e) => {
                        println!("2. plan for {} failed: {e}", mode.as_str());
                        ok = false;
                        continue;
                    }
                };
                let has_root = plan
                    .partitions
                    .iter()
                    .any(|p| p.purpose == camarelol_core::storage::PartitionPurpose::Root);
                let has_esp = plan
                    .partitions
                    .iter()
                    .any(|p| p.purpose == camarelol_core::storage::PartitionPurpose::EfiSystem);
                // UEFI needs an ESP, BIOS must not have one.
                let correct = has_root && (matches!(mode, Firmware::Uefi) == has_esp);
                println!(
                    "2. {} -> {} partitions (root: {has_root}, esp: {has_esp}, table: {}) {}",
                    mode.as_str(),
                    plan.partitions.len(),
                    plan.table_type,
                    if correct { "ok" } else { "WRONG" },
                );
                ok = ok && correct;
            }
            println!(
                "   detected firmware on this machine: {}",
                detected.as_str()
            );
        }
        None => println!("2. no empty disk to plan for (skipped)"),
    }

    // B. Windows detection when present
    for d in core.devices.iter().filter(|d| d.has_windows()) {
        println!(
            "3. windows detected on {} — risk {:?}; camareLOL will never format its ESP silently",
            d.path, d.risk
        );
    }

    // C. destructive gate is closed by default
    let gate = SafetyGate::default();
    let rejected = gate
        .authorize(
            camarelol_core::safety::Operation::FormatPartition,
            camarelol_core::storage::RiskLevel::High,
        )
        .is_err();
    println!("4. safety gate refuses formatting by default: {rejected}");
    ok = ok && rejected;

    // D. config round trip
    let cfg = InstallationConfig::default();
    let path = std::env::temp_dir().join("camarelol-selftest.toml");
    cfg.save(path.to_str().unwrap_or_default())?;
    let loaded = InstallationConfig::load(path.to_str().unwrap_or_default())?;
    println!("5. configuration round trip: ok ({})", loaded.hostname);
    ok = ok && loaded.hostname == cfg.hostname;

    // E. full simulated install
    if let Some(d) = core.devices.first() {
        let plan = core.plan_for(&d.path)?;
        // run_install() resolves the firmware from the profile; pin it to the
        // mode the plan was actually built for, otherwise the preflight would
        // reject a UEFI plan on a BIOS host (and vice versa).
        let cfg = InstallationConfig {
            firmware: match core.firmware() {
                Firmware::Uefi => "uefi",
                Firmware::Bios => "bios",
                Firmware::Unknown => "auto",
            }
            .to_string(),
            ..Default::default()
        };
        let mut runner = SimulatedRunner::new();
        let mut sink = NullSink;
        let log = Logger::default();
        let gate = SafetyGate::armed_and_execute();
        let report = run_install(&cfg, d, &plan, &gate, &log, &mut runner, &mut sink);
        println!(
            "6. simulated install: {} ({} phases)",
            report.outcome.as_str(),
            report.phases.len()
        );
        ok = ok && matches!(report.outcome, Outcome::Completed);
    }

    println!("selftest result: {}", if ok { "PASS" } else { "FAIL" });
    Ok(ok)
}

fn run_tui(args: Args) -> Result<(), CamarelolError> {
    let mut app = App::new(args.mock.clone());
    if let Some(lang) = &args.language {
        app.language_index = if lang == "vi" { 0 } else { 1 };
        app.sync_language_to_system();
    }
    if let Some(cfg_path) = &args.config {
        let cfg = InstallationConfig::load(cfg_path.to_str().unwrap_or_default())?;
        app.core.config = cfg;
    }
    app.armed = args.allow_destructive;
    app.execute_mode = args.execute;
    app.save_config_path = args.save_config;

    let mut terminal = ratatui::init();
    let result = (|| -> Result<(), CamarelolError> {
        loop {
            terminal
                .draw(|f| draw::draw(f, &mut app))
                .map_err(|e| CamarelolError::new(ErrorCode::Internal, e.to_string()))?;
            app.poll_update_check();
            if app.quit {
                break;
            }
            // Blocking read keeps things simple and CPU idle.
            let event = crossterm::event::read()
                .map_err(|e| CamarelolError::new(ErrorCode::Internal, e.to_string()))?;
            if let crossterm::event::Event::Key(key) = event {
                app.handle_key(key);
            }
        }
        Ok(())
    })();
    ratatui::restore();
    result
}
