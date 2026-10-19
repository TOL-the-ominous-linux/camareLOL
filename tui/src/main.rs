//! camareLOL — a modern, safety-first TUI installer for Arch Linux.

mod app;
mod draw;
mod keys;
mod scripted;
mod version;

use crate::app::App;
use camarelol_core::config::InstallationConfig;
use camarelol_core::error::{CamarelolError, ErrorCode};
use camarelol_core::hardware::Firmware;
use camarelol_core::install::execute::SimulatedRunner;
use camarelol_core::install::run_install;
use camarelol_core::install::{NullSink, Outcome};
use camarelol_core::locale::UiLanguage;
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
    /// Simulate a complete installation: all twelve phases, every command
    /// printed, nothing written and nothing run.
    preview: bool,
    scan: bool,
    self_test: bool,
    scripted: Option<PathBuf>,
    language: Option<String>,
    postcheck: bool,
    postcheck_root: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    parse_args_from(argv)
}

/// Split out from [`parse_args`] so the flag handling is testable without
/// spawning the binary with a doctored `argv`.
fn parse_args_from(argv: Vec<String>) -> Result<Args, String> {
    let mut a = Args::default();
    let mut args = argv.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--mock" => a.mock = args.next().map(PathBuf::from),
            "--config" => a.config = args.next().map(PathBuf::from),
            "--save-config" => a.save_config = args.next().map(PathBuf::from),
            "--allow-destructive" => a.allow_destructive = true,
            "--execute" => a.execute = true,
            "--preview" => a.preview = true,
            "--scan" => a.scan = true,
            "--self-test" => a.self_test = true,
            "--scripted" => a.scripted = args.next().map(PathBuf::from),
            "--lang" => a.language = args.next(),
            "--postcheck" => a.postcheck = true,
            "--postcheck-root" => a.postcheck_root = args.next().map(PathBuf::from),
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
    --preview                 Simulate the whole installation and write nothing
    --allow-destructive       Arm the safety gate (still requires confirmation)
    --execute                 Really write to disk (default is dry-run)
    --lang <vi|en|zh>         Installer language
    --postcheck               Verify an installed system (default: /)
    --postcheck-root <PATH>   Root to check (for mounted systems)

    -h, --help                Show this help
    -V, --version             Show version and commit

PREVIEW:
    --preview walks all twelve phases and prints the exact commands a real
    install would run. It starts none of them and writes nothing: the safety
    latch in the core refuses to spawn any changing command for the rest of
    the run. --allow-destructive and --execute are ignored next to it, so
    `--preview --execute` is still a preview.

UPDATING:
    camareLOL does not update itself. There is no updater, no index URL and
    no code path that replaces this binary.

    That is a decision, not an omission. This program erases disks; a component
    whose job is to replace itself is the shortest path from a compromised
    server to erased data. There is no pacman package and no AUR package —
    re-run the one-liner; it picks the newest release and verifies it:

        curl -fsSL https://raw.githubusercontent.com/tamkogay9876-code/camareLOL/main/scripts/camarelol.sh | bash

    Or download the release you want from
    https://github.com/tamkogay9876-code/camareLOL/releases and replace the
    binary by hand.
"
}

/// The startup warning. Printed on stderr, before anything is drawn, so it is
/// the first thing a user sees even if the interface never comes up.
fn preview_warning() -> String {
    "\
╔══════════════════════════════════════════════════════════════════════════╗
║  PREVIEW — KHÔNG GÌ ĐƯỢC GHI VÀO ĐĨA — NOTHING WILL BE WRITTEN TO DISK  ║
╚══════════════════════════════════════════════════════════════════════════╝
  Toàn bộ 12 pha sẽ chạy và in ra từng lệnh thật. Không lệnh nào được chạy,
  không byte nào được ghi. --allow-destructive và --execute bị bỏ qua.

  All twelve phases run and every real command is printed. Nothing is executed
  and nothing is written; --allow-destructive and --execute are ignored."
        .to_string()
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

    // Chốt chặn an toàn trước mọi thứ khác, kể cả trước lần quét đĩa đầu
    // tiên: từ thời điểm này `sys` sẽ từ chối mọi lệnh có thể thay đổi máy,
    // không cần lớp nào phía trên phải nhớ kiểm tra.
    if args.preview {
        camarelol_core::sys::enter_preview();
        eprintln!("{}", preview_warning());
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

    if args.postcheck {
        let root = args
            .postcheck_root
            .clone()
            .unwrap_or_else(|| PathBuf::from("/"));
        cmd_postcheck(&root);
        return;
    }

    if let Some(cfg_path) = args.scripted.clone() {
        match scripted::run_scripted(
            &cfg_path,
            args.mock.as_deref(),
            args.allow_destructive && args.execute,
            args.preview,
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

    // The TUI needs a terminal. Without this check ratatui panics from inside
    // its own initialiser, and the message a user sees names a crate version
    // and a registry path instead of what they should do. Running it over a
    // pipe, from cron, or in CI all land here.
    if !std::io::IsTerminal::is_terminal(&std::io::stdout()) {
        eprintln!(
            "camareLOL needs a terminal to draw its interface, and there is none here.\n\
             \n\
             Run it from a console — the Arch live ISO is the intended place:\n\
             \x20   sudo camarelol\n\
             \n\
             For unattended runs there is no interface to draw, so use the scripted path:\n\
             \x20   camarelol --scripted <config.toml>\n\
             and add --allow-destructive --execute to actually write to a disk."
        );
        std::process::exit(1);
    }

    match run_tui(args) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("camareLOL failed: {e}");
            std::process::exit(1);
        }
    }
}

/// Verify an installed system: check that the features the installer promised
/// are actually on disk. Each check is a direct file existence test — the same
/// markers the VM acceptance harness uses in its step 6.
fn cmd_postcheck(root: &std::path::Path) {
    let checks = [
        (
            "zram_unit",
            root.join("usr/lib/systemd/system/systemd-zram-setup@.service"),
        ),
        (
            "zram_wants",
            root.join(
                "etc/systemd/system/multi-user.target.wants/systemd-zram-setup@zram0.service",
            ),
        ),
        (
            "zram_bin",
            root.join("usr/lib/systemd/system-generators/zram-generator"),
        ),
        ("zram_conf", root.join("etc/systemd/zram-generator.conf")),
        (
            "sshd_unit",
            root.join("usr/lib/systemd/system/sshd.service"),
        ),
        (
            "sshd_wants",
            root.join("etc/systemd/system/multi-user.target.wants/sshd.service"),
        ),
        ("openssh", root.join("usr/bin/sshd")),
    ];
    let mut failed = 0;
    for (name, path) in &checks {
        if path.exists() {
            println!("  PASS {name}");
        } else {
            println!("  FAIL {name} — {}", path.display());
            failed += 1;
        }
    }
    if failed > 0 {
        println!("  {failed} check(s) failed");
        std::process::exit(1);
    }
    println!("  all checks passed");
}

/// Rust ignores SIGPIPE and turns it into a panic on the next write, so
/// instead of stopping quietly like every other command-line tool.
fn restore_default_sigpipe() {
    // SAFETY: setting a signal disposition to SIG_DFL is async-signal-safe and
    // is done before any other thread exists.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(unix)]
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
    // Host identity first and explicitly. Every later check assumes Arch +
    // systemd, so a "self-test failed" with no mention of the host would send
    // the user hunting for a bug that is actually their distribution.
    let host = camarelol_core::host::detect();
    println!("0. host detected: {}", host.summary());
    println!("   supported: {}", host.supported);
    if !host.supported {
        println!("   REFUSED: {}", host.reason);
        ok = false;
    }
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
        // Unknown codes stay English rather than being guessed at: a
        // mistyped `--lang zh-chn` must not silently install the wrong system
        // locale on the user's machine.
        if let Some(l) = UiLanguage::from_code(lang) {
            app.language_index = l.index();
            app.sync_language_to_system();
        }
    }
    if let Some(cfg_path) = &args.config {
        let cfg = InstallationConfig::load(cfg_path.to_str().unwrap_or_default())?;
        app.core.config = cfg;
    }
    app.armed = args.allow_destructive;
    app.execute_mode = args.execute;
    // Set last and unconditionally: `--preview` overrides both flags above.
    app.preview = args.preview;
    app.save_config_path = args.save_config;

    let mut terminal = ratatui::init();
    let result = (|| -> Result<(), CamarelolError> {
        loop {
            // Gom cập nhật từ thread cài đặt TRƯỚC khi vẽ, để màn Progress
            // hiện ngay khi có pha mới thay vì đứng yên.
            app.drain_install_updates();
            terminal
                .draw(|f| draw::draw(f, &mut app))
                .map_err(|e| CamarelolError::new(ErrorCode::Internal, e.to_string()))?;
            if app.quit {
                break;
            }
            // Poll thay vì read() chặn: có thời hạn, nên vẽ và nhận cập nhật
            // được kể cả khi người dùng không gõ gì.
            let ready = crossterm::event::poll(std::time::Duration::from_millis(120))
                .map_err(|e| CamarelolError::new(ErrorCode::Internal, e.to_string()))?;
            if !ready {
                continue;
            }
            if let crossterm::event::Event::Key(key) = crossterm::event::read()
                .map_err(|e| CamarelolError::new(ErrorCode::Internal, e.to_string()))?
            {
                app.handle_key(key);
            }
        }
        Ok(())
    })();
    ratatui::restore();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(argv: &[&str]) -> Args {
        parse_args_from(argv.iter().map(|s| s.to_string()).collect()).unwrap()
    }

    #[test]
    fn preview_is_off_unless_asked_for() {
        assert!(!parse(&[]).preview);
        assert!(!parse(&["--allow-destructive", "--execute"]).preview);
        assert!(parse(&["--preview"]).preview);
    }

    #[test]
    fn preview_is_the_only_flag_that_can_say_no_to_the_others() {
        // A wrapper script that always appends the destructive flags must not be
        // able to defeat --preview. `App::gate()` is the place that guarantees
        // it; this pins the flag itself so a future parser change cannot turn
        // the two into an either/or.
        let args = parse(&["--preview", "--allow-destructive", "--execute"]);
        assert!(args.preview && args.allow_destructive && args.execute);

        let mut app = App::new(None);
        app.preview = args.preview;
        app.armed = args.allow_destructive;
        app.execute_mode = args.execute;
        let gate = app.gate();
        assert!(gate.preview);
        assert!(!gate.writes_to_disk(), "the preview flag must win");
        assert_eq!(
            gate.mode(),
            camarelol_core::safety::GateMode::Preview,
            "the footer must not say EXECUTE"
        );

        // Without --preview the very same combination is a real, armed install.
        let plain = parse(&["--allow-destructive", "--execute"]);
        let mut real = App::new(None);
        real.preview = plain.preview;
        real.armed = plain.allow_destructive;
        real.execute_mode = plain.execute;
        assert!(real.gate().writes_to_disk());
    }

    #[test]
    fn the_startup_warning_says_nothing_is_written() {
        let text = preview_warning();
        assert!(text.contains("PREVIEW"), "{text}");
        assert!(text.contains("KHÔNG GÌ ĐƯỢC GHI VÀO ĐĨA"), "{text}");
        assert!(text.contains("--allow-destructive"), "{text}");
    }

    #[test]
    fn usage_documents_preview() {
        let u = usage();
        assert!(u.contains("--preview"), "the help must list the flag");
        assert!(u.contains("writes nothing"), "and say what it does:\n{u}");
        assert!(
            u.contains("--execute is ignored") || u.contains("are ignored"),
            "and that it cannot be combined with a real run:\n{u}"
        );
    }

    #[test]
    fn unknown_arguments_are_still_rejected() {
        let err = parse_args_from(vec!["--preview-everything".to_string()]).unwrap_err();
        assert!(err.contains("unknown argument"), "{err}");
    }
}
