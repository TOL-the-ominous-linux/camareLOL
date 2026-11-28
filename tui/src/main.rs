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
    /// Write a redacted diagnostics report here. Works from the command line
    /// (it is one of the actions that needs no terminal) and from the TUI,
    /// where `d` on the result screen writes to the same path.
    save_diagnostics: Option<PathBuf>,
    allow_destructive: bool,
    execute: bool,
    /// Simulate a complete installation: all twelve phases, every command
    /// printed, nothing written and nothing run.
    preview: bool,
    scan: bool,
    self_test: bool,
    scripted: Option<PathBuf>,
    /// Headless installs have no screen to type a LUKS passphrase into, and it
    /// must not be in the profile (nor in argv, where `ps` would show it).
    luks_passphrase_file: Option<PathBuf>,
    /// Same as [`Args::luks_passphrase_file`], for the root account: the
    /// profiles the wizard writes never carry a password, and a scripted run
    /// needs one for the acceptance test.
    root_password_file: Option<PathBuf>,
    language: Option<String>,
    postcheck: bool,
    postcheck_root: Option<PathBuf>,
    aur_preflight: bool,
    plan_only: Option<PathBuf>,
    json: bool,
}

/// Read a LUKS passphrase from the first line of a file.
///
/// Only the newline is stripped: a passphrase may legitimately have leading or
/// trailing spaces, and trimming them would produce a container nobody can
/// open with the passphrase the user wrote down.
fn read_passphrase_file(path: &std::path::Path) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        format!(
            "cannot read the LUKS passphrase file {}: {e}",
            path.display()
        )
    })?;
    let first = text.lines().next().unwrap_or_default().to_string();
    if first.is_empty() {
        return Err(format!(
            "the LUKS passphrase file {} is empty",
            path.display()
        ));
    }
    Ok(first)
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
            "--save-diagnostics" => a.save_diagnostics = args.next().map(PathBuf::from),
            "--allow-destructive" => a.allow_destructive = true,
            "--execute" => a.execute = true,
            "--preview" => a.preview = true,
            "--scan" => a.scan = true,
            "--self-test" => a.self_test = true,
            "--scripted" => a.scripted = args.next().map(PathBuf::from),
            "--luks-passphrase-file" => a.luks_passphrase_file = args.next().map(PathBuf::from),
            "--root-password-file" => a.root_password_file = args.next().map(PathBuf::from),
            "--lang" => a.language = args.next(),
            "--postcheck" => a.postcheck = true,
            "--postcheck-root" => a.postcheck_root = args.next().map(PathBuf::from),
            "--aur-preflight" => a.aur_preflight = true,
            "--plan-only" => a.plan_only = args.next().map(PathBuf::from),
            "--json" => a.json = true,
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
    --save-diagnostics <FILE> Write a redacted diagnostics report there
    --mock <FILE>             Use a JSON device survey instead of real disks
    --scan                    Print the storage survey as JSON and exit
    --self-test               Run the non-destructive acceptance checks
    --scripted <FILE>         Install without a UI (for automated testing)
    --luks-passphrase-file <FILE>
                              Read the LUKS passphrase from the first line of
                              FILE (scripted installs of an encrypted root)
    --root-password-file <FILE>
                              Read the root password from the first line of
                              FILE (scripted installs; profiles never carry
                              a password)
    --preview                 Simulate the whole installation and write nothing
    --allow-destructive       Arm the safety gate (still requires confirmation)
    --execute                 Really write to disk (default is dry-run)
    --lang <vi|en|zh>         Installer language
    --postcheck               Verify an installed system (default: /)
    --postcheck-root <PATH>   Root to check (for mounted systems)
    --aur-preflight            Check AUR build prerequisites and exit
    --plan-only <FILE>         Load a profile, plan its disk layout, and exit
                               (no writes; prints the plan as text and JSON)
    --json                    Emit machine-readable JSON for --postcheck and
                               --aur-preflight

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

        curl -fsSL https://raw.githubusercontent.com/TOL-the-ominous-linux/camareLOL/main/scripts/camarelol.sh | bash

    Or download the release you want from
    https://github.com/TOL-the-ominous-linux/camareLOL/releases and replace the
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
            Ok(SelfTestOutcome::Pass) => {
                println!("self-test: OK");
                std::process::exit(0);
            }
            // Exit 2: the environment is not a machine camareLOL supports.
            // Distinct from 1 so scripts and CI can tell "wrong distro"
            // apart from "the tool failed one of its own checks".
            Ok(SelfTestOutcome::SkipOnHost) => {
                println!("self-test: SKIP (host unsupported)");
                std::process::exit(2);
            }
            Ok(SelfTestOutcome::Fail) => {
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
        cmd_postcheck(&root, args.json);
        return;
    }

    if args.aur_preflight {
        cmd_aur_preflight(args.json);
        return;
    }

    if let Some(path) = args.plan_only.clone() {
        cmd_plan_only(&path, args.mock.clone());
        return;
    }

    // Diagnostics before the terminal check, because a user who wants to attach
    // a report to a bug report is very often not sitting at a console — they
    // are pasting into an issue tracker from another machine.
    if let Some(path) = args.save_diagnostics.clone() {
        match cmd_save_diagnostics(&path) {
            Ok(written) => println!("diagnostics report written to {}", written.display()),
            Err(e) => {
                eprintln!("cannot write the diagnostics report: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    if let Some(cfg_path) = args.scripted.clone() {
        let passphrase = match args.luks_passphrase_file.as_deref() {
            Some(path) => match read_passphrase_file(path) {
                Ok(p) => Some(p),
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            },
            None => None,
        };
        let root_password = match args.root_password_file.as_deref() {
            Some(path) => match read_passphrase_file(path) {
                Ok(p) => Some(p),
                Err(e) => {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
            },
            None => None,
        };
        match scripted::run_scripted(
            &cfg_path,
            args.mock.as_deref(),
            args.allow_destructive && args.execute,
            args.preview,
            passphrase.as_deref(),
            root_password.as_deref(),
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
fn cmd_postcheck(root: &std::path::Path, json: bool) {
    // (name, status, detail)
    let mut results: Vec<(&str, &str, String)> = Vec::new();

    let required = [
        ("zram_unit", root.join("usr/lib/systemd/system/systemd-zram-setup@.service")),
        ("zram_wants", root.join("etc/systemd/system/multi-user.target.wants/systemd-zram-setup@zram0.service")),
        ("zram_bin", root.join("usr/lib/systemd/system-generators/zram-generator")),
        ("zram_conf", root.join("etc/systemd/zram-generator.conf")),
        ("sshd_unit", root.join("usr/lib/systemd/system/sshd.service")),
        ("sshd_wants", root.join("etc/systemd/system/multi-user.target.wants/sshd.service")),
        ("openssh", root.join("usr/bin/sshd")),
    ];
    for (name, path) in &required {
        if path.exists() {
            results.push((name, "PASS", path.display().to_string()));
        } else {
            results.push((name, "FAIL", path.display().to_string()));
        }
    }

    // bootloader: GRUB or systemd-boot
    let grub = root.join("boot/grub/grub.cfg");
    let sdboot = root.join("boot/loader/loader.conf");
    if grub.exists() || sdboot.exists() {
        results.push(("bootloader", "PASS", format!("grub={}, sdboot={}", grub.exists(), sdboot.exists())));
    } else {
        results.push(("bootloader", "FAIL", "neither grub.cfg nor loader.conf found".to_string()));
    }

    // initramfs
    let initramfs_candidates = [
        "boot/initramfs-linux.img",
        "boot/initramfs-linux-fallback.img",
        "boot/initramfs-linux-lts.img",
    ];
    if initramfs_candidates.iter().any(|p| root.join(p).exists()) {
        results.push(("initramfs", "PASS", "present".to_string()));
    } else {
        results.push(("initramfs", "FAIL", "no initramfs-linux*.img found".to_string()));
    }

    // crypttab: SKIP if missing/empty (unencrypted install)
    let crypttab = root.join("etc/crypttab");
    if crypttab.exists() {
        let content = std::fs::read_to_string(&crypttab).unwrap_or_default();
        if content.trim().is_empty() {
            results.push(("crypttab", "SKIP", "empty".to_string()));
        } else if content.contains("cryptroot") {
            results.push(("crypttab", "PASS", "cryptroot present".to_string()));
        } else {
            results.push(("crypttab", "FAIL", "content check failed".to_string()));
        }
    } else {
        results.push(("crypttab", "SKIP", "not present".to_string()));
    }

    // NetworkManager config optional
    let nm = root.join("etc/NetworkManager/conf.d/00-camarelol.conf");
    if nm.exists() {
        results.push(("networkmanager_config", "PASS", nm.display().to_string()));
    } else {
        results.push(("networkmanager_config", "SKIP", "not present".to_string()));
    }

    // greetd config optional
    let greetd = root.join("etc/greetd/config.toml");
    if greetd.exists() {
        let ok = std::fs::read_to_string(&greetd).map(|s| s.contains("command")).unwrap_or(false);
        if ok {
            results.push(("greetd_config", "PASS", greetd.display().to_string()));
        } else {
            results.push(("greetd_config", "FAIL", "content check failed".to_string()));
        }
    } else {
        results.push(("greetd_config", "SKIP", "not present".to_string()));
    }

    if json {
        // Emit a JSON array for tooling.
        let arr: Vec<serde_json::Value> = results
            .iter()
            .map(|(n, st, d)| {
                serde_json::json!({"name": n, "status": st, "detail": d})
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_else(|e| e.to_string()));
    } else {
        for (name, status, detail) in &results {
            println!("  {} {} — {}", status, name, detail);
        }
    }
    // Count FAILs (SKIP does not fail).
    let fail_count = results.iter().filter(|(_, s, _)| *s == "FAIL").count();
    if fail_count > 0 {
        println!("  {} check(s) failed", fail_count);
        std::process::exit(1);
    }
    if !json {
        println!("  all checks passed");
    }
}

fn cmd_aur_preflight(json: bool) {
    use camarelol_core::aur;
    let items = aur::preflight();
    if json {
        let arr: Vec<serde_json::Value> = items
            .iter()
            .map(|i| {
                serde_json::json!({"name": i.name, "ok": i.ok, "detail": i.detail})
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_else(|e| e.to_string()));
    } else {
        println!("AUR preflight:");
        for item in &items {
            if item.ok {
                println!("  PASS {} — {}", item.name, item.detail);
            } else {
                println!("  FAIL {} — {}", item.name, item.detail);
            }
        }
    }
    if items.iter().any(|i| !i.ok) {
        if !json {
            println!("  {} prerequisite(s) missing", items.iter().filter(|i| !i.ok).count());
        }
        std::process::exit(1);
    }
    if !json {
        println!("  all AUR prerequisites present");
    }
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
fn cmd_plan_only(config_path: &std::path::Path, mock: Option<PathBuf>) {
    use camarelol_core::config::InstallationConfig;
    use camarelol_core::storage::{scan_devices, scan_devices_from_survey, verify_against};
    let cfg = match InstallationConfig::load(config_path.to_str().unwrap_or_default()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error loading profile: {e}");
            std::process::exit(2);
        }
    };
    if let Err(e) = cfg.validate() {
        eprintln!("profile validation failed: {e}");
        std::process::exit(2);
    }
    let devices = match mock {
        Some(p) => match std::fs::read_to_string(&p) {
            Ok(text) => match scan_devices_from_survey(&text) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("error reading mock survey: {e}");
                    std::process::exit(2);
                }
            },
            Err(e) => {
                eprintln!("cannot read mock survey: {e}");
                std::process::exit(2);
            }
        },
        None => match scan_devices() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("error scanning devices: {e}");
                std::process::exit(2);
            }
        },
    };
    let target = match cfg.target_disk.clone() {
        Some(t) => t,
        None => {
            eprintln!("profile has no target_disk");
            std::process::exit(2);
        }
    };
    let device = match verify_against(&target.path, &target.identity, target.size_bytes, &devices) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("target disk verification failed: {e}");
            std::process::exit(2);
        }
    };
    let mut core = camarelol_core::Core::with_devices(devices);
    core.config = cfg.clone();
    if let Ok(fw) = cfg.resolve_firmware(camarelol_core::hardware::detect_firmware()) {
        core.set_firmware(fw);
    }
    if let Ok(bl) = cfg.bootloader() {
        core.set_bootloader(bl);
    }
    let plan = match core.plan_for(&device.path) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("planning failed: {e}");
            std::process::exit(2);
        }
    };
    if let Err(e) = core.validate_plan(&plan) {
        eprintln!("plan validation failed: {e}");
        std::process::exit(2);
    }
    println!("{}", plan.render());
    match serde_json::to_string_pretty(&plan) {
        Ok(json) => {
            println!("--- plan.json ---");
            println!("{json}");
        }
        Err(e) => eprintln!("could not serialise plan: {e}"),
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

/// Outcome of `--self-test`. A machine camareLOL never supported is not a
/// failure of the tool: it reports [`SelfTestOutcome::SkipOnHost`] and the
/// caller exits 2, so a script can tell "wrong environment" apart from
/// "the tool is broken".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelfTestOutcome {
    Pass,
    SkipOnHost,
    Fail,
}

/// Non-destructive acceptance checks derived from the specification.
fn cmd_self_test(mock: Option<PathBuf>) -> Result<SelfTestOutcome, CamarelolError> {
    let devices = match mock {
        Some(p) => camarelol_core::storage::load_survey(p.to_str().unwrap_or_default())?,
        None => scan_devices()?,
    };
    let core = camarelol_core::Core::with_devices(devices);
    let mut ok = true;
    // Only the host gate can turn a failing run into a skip. Every other
    // check exercises camareLOL's own machinery, wherever it runs.
    let mut host_bad = false;
    let empty = core.devices.iter().find(|d| d.is_empty());
    // Host identity first and explicitly. Every later check assumes Arch +
    // systemd, so a "self-test failed" with no mention of the host would send
    // the user hunting for a bug that is actually their distribution.
    let host = camarelol_core::host::detect();
    println!("0. host detected: {}", host.summary());
    println!("   supported: {}", host.supported);
    if !host.supported {
        println!("   REFUSED: {}", host.reason);
        host_bad = true;
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
    if let Some(d) = empty {
        if !host.supported {
            // The simulated install still runs the preflight, which refuses an
            // unsupported host. That is the host gate again, not broken
            // machinery, so it must not turn the SKIP into a FAIL.
            println!("6. simulated install: skipped (host unsupported)");
        } else {
            let plan = core.plan_for(&d.path)?;
            // run_install() resolves the firmware from the profile; pin it to
            // the mode the plan was actually built for, otherwise the
            // preflight would reject a UEFI plan on a BIOS host (and vice
            // versa).
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
    } else {
        // Most installed machines have no empty disk, and that is not a
        // failure: the simulated install needs a target it may format. Using
        // `devices.first()` here made --self-test fail on every normal
        // desktop (and picked zram on hosts that have it).
        println!("6. simulated install: skipped (no empty disk)");
    }

    println!("7. AUR preflight (informational):");
    for item in camarelol_core::aur::preflight() {
        println!(
            "   - {}: {} ({})",
            item.name,
            if item.ok { "ok" } else { "missing" },
            item.detail
        );
    }

    if !ok {
        println!("selftest result: FAIL");
        println!(
            "   machinery broke on a host it runs anywhere — this is a bug, please report it."
        );
        return Ok(SelfTestOutcome::Fail);
    }
    if host_bad {
        println!("selftest result: SKIP (host unsupported)");
        println!("   the machinery checks above still ran: this says nothing bad about camareLOL.");
        return Ok(SelfTestOutcome::SkipOnHost);
    }
    println!("selftest result: PASS");
    Ok(SelfTestOutcome::Pass)
}

/// Write a redacted diagnostics report and return where it went.
///
/// Everything in the report passes through the core's redaction pass, so this
/// is safe to attach to a public issue: no password, token, key or Wi-Fi
/// passphrase can be in it. The function says so in its own output rather than
/// leaving the user to take it on trust.
fn cmd_save_diagnostics(path: &std::path::Path) -> Result<PathBuf, CamarelolError> {
    let mut input = camarelol_core::recovery::collect_input();
    input.mode = "diagnostics only — nothing was installed".to_string();
    camarelol_core::recovery::export_text(path.to_str().unwrap_or_default(), &input)?;
    println!("  redacted: no password, token, key or passphrase is in this file.");
    Ok(path.to_path_buf())
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
    // `--config` goes through the *same* load path as pressing `L` on the
    // welcome screen, so the wizard really is populated. Assigning
    // `app.core.config` instead used to load a profile that the wizard then
    // overwrote from its own defaults on the first keypress — the feature
    // existed and did nothing.
    if let Some(cfg_path) = &args.config {
        app.load_profile(cfg_path.to_str().unwrap_or_default())?;
    }
    app.armed = args.allow_destructive;
    app.execute_mode = args.execute;
    // Set last and unconditionally: `--preview` overrides both flags above.
    app.preview = args.preview;
    app.save_config_path = args.save_config;
    app.diagnostics_path = args.save_diagnostics;

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
