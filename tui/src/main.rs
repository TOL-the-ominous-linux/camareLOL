//! camareLOL — a modern, safety-first TUI installer for Arch Linux.

mod app;
mod draw;
mod scripted;
mod update;

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
            "-h" | "--help" => {
                println!("{}", usage());
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("camareLOL {}", env!("CARGO_PKG_VERSION"));
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
    -h, --help                Show this help
    -V, --version             Show version
"
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("{}", usage());
            std::process::exit(2);
        }
    };

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

    // A. empty disk planning
    match empty {
        Some(d) => {
            let plan = core.plan_for(&d.path)?;
            core.validate_plan(&plan)?;
            let has_root = plan
                .partitions
                .iter()
                .any(|p| p.purpose == camarelol_core::storage::PartitionPurpose::Root);
            let has_esp = plan
                .partitions
                .iter()
                .any(|p| p.purpose == camarelol_core::storage::PartitionPurpose::EfiSystem);
            let firmware = core.firmware();
            println!(
                "2. empty disk -> {} partitions (root: {has_root}, esp: {has_esp}, firmware {})",
                plan.partitions.len(),
                firmware.as_str()
            );
            if !matches!(firmware, Firmware::Uefi) || has_esp {
                ok = ok && has_root;
            } else {
                ok = false;
            }
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
