//! Headless installation mode used by the VirtualBox acceptance test.
//!
//! The same core, plan and safety checks are used — only the UI is missing.

use camarelol_core::config::InstallationConfig;
use camarelol_core::error::{CamarelolError, ErrorCode};
use camarelol_core::hardware::detect_firmware;
use camarelol_core::install::execute::{InstallReport, RealSystemRunner};
use camarelol_core::install::{run_install, Outcome, ProgressEvent, ProgressSink};
use camarelol_core::logging::Logger;
use camarelol_core::safety::{required_phrase, SafetyGate};
use camarelol_core::storage::{scan_devices, scan_devices_from_survey};
use std::path::Path;

struct TextSink;

impl ProgressSink for TextSink {
    fn emit(&mut self, event: ProgressEvent) {
        match event {
            ProgressEvent::PhaseStarted { name } => {
                println!(
                    "▶ {name} — {}",
                    camarelol_core::install::describe_phase(&name)
                )
            }
            ProgressEvent::PhaseFinished { name, state } => {
                println!("■ {name} — {}", state.as_str())
            }
            ProgressEvent::CommandPreview { command } => println!("  $ {command}"),
            ProgressEvent::Info { message } => println!("  · {message}"),
        }
    }
}

/// Install from a saved configuration file.
///
/// * `allow_execute` must be true — dry runs are reported instead of run.
/// * `preview` simulates a *complete* install: all twelve phases run and every
///   command is printed, but nothing is written and nothing is executed. It
///   ignores `allow_execute` completely.
/// * The target disk is re-scanned and verified before anything is written.
pub fn run_scripted(
    config_path: &Path,
    mock: Option<&Path>,
    allow_execute: bool,
    preview: bool,
) -> Result<InstallReport, CamarelolError> {
    let cfg = InstallationConfig::load(config_path.to_str().unwrap_or_default())?;
    cfg.validate()?;

    let devices = match mock {
        Some(p) => scan_devices_from_survey(&std::fs::read_to_string(p).map_err(|e| {
            CamarelolError::new(
                ErrorCode::ParseFailed,
                format!("cannot read mock survey: {e}"),
            )
        })?)?,
        None => scan_devices()?,
    };

    let target = cfg.target_disk.clone().ok_or_else(|| {
        CamarelolError::new(
            ErrorCode::NoTargetDisk,
            "the configuration has no target_disk; run the TUI to choose one",
        )
        .with_remediation("set target_disk = { path, identity, size_bytes } in the config")
    })?;

    // Verify the disk identity strictly — a config from another machine is
    // never trusted.
    let device = camarelol_core::storage::verify_against(
        &target.path,
        &target.identity,
        target.size_bytes,
        &devices,
    )?;

    let mut core = camarelol_core::Core::with_devices(devices);
    core.config = cfg.clone();
    core.set_firmware(cfg.resolve_firmware(detect_firmware())?);
    core.set_bootloader(cfg.bootloader()?);

    let plan = core.plan_for(&device.path)?;
    core.validate_plan(&plan)?;

    // A preview runs the real orchestrator, so it gets its own branch here: it
    // is not the "dry-run" of the block below, which only prints the partition
    // plan and returns before any phase happens.
    if preview {
        println!("PREVIEW — KHÔNG GÌ ĐƯỢC GHI VÀO ĐĨA / nothing will be written to disk");
        println!(
            "simulating all phases for {} ({})",
            device.path,
            device.identity()
        );
        println!("Plan:\n{}", plan.render());
        let gate = SafetyGate::preview();
        let log = Logger::default();
        let mut sink = TextSink;
        let mut runner = RealSystemRunner::new(gate.clone());
        let report = run_install(&cfg, &device, &plan, &gate, &log, &mut runner, &mut sink);
        println!("\nCommands that would run:");
        for line in runner.command_lines() {
            println!("  {line}");
        }
        println!();
        println!("{}", report.render());
        println!(
            "preview completed — {} commands recorded, 0 executed.",
            runner.records().len()
        );
        return Ok(report);
    }

    if !allow_execute {
        println!("dry-run: nothing was written. Re-run with --allow-destructive --execute.");
        println!("Plan:\n{}", plan.render());
        let mut ops = camarelol_core::storage::ops::OpRunner::new();
        let log = Logger::default();
        ops.apply_partition_table(&device, &plan, &SafetyGate::armed(), &log)?;
        println!("Commands that would run:\n{}", ops.report());
        return Ok(InstallReport {
            phases: vec![],
            outcome: Outcome::Completed,
            warnings: vec!["dry-run".into()],
        });
    }

    println!(
        "about to write to {} ({}). confirmation phrase: {}",
        device.path,
        device.identity(),
        required_phrase(plan.disk_risk)
    );

    // In scripted mode the caller has already armed the gate; we still record
    // the confirmation so the gate can never be bypassed by accident.
    let mut gate = SafetyGate::armed_and_execute();
    gate.confirm(
        camarelol_core::safety::Operation::WritePartitionTable,
        plan.disk_risk,
        required_phrase(plan.disk_risk),
    )?;
    // Record the confirmation for every operation the install performs, at every
    // risk level it can use. Confirmations are keyed by (operation, risk), so
    // a partial list would stop the run later with a confusing error.
    for op in [
        camarelol_core::safety::Operation::WritePartitionTable,
        camarelol_core::safety::Operation::FormatPartition,
        camarelol_core::safety::Operation::InstallBootloader,
        camarelol_core::safety::Operation::Pacstrap,
        camarelol_core::safety::Operation::Mount,
    ] {
        for risk in [
            camarelol_core::storage::RiskLevel::Low,
            camarelol_core::storage::RiskLevel::Medium,
            camarelol_core::storage::RiskLevel::High,
        ] {
            gate.confirm(op, risk, required_phrase(risk))?;
        }
    }

    let log = Logger::new(std::path::Path::new("/var/log/camarelol"));
    let mut sink = TextSink;
    let mut runner = RealSystemRunner::new(gate.clone());
    let report = run_install(&cfg, &device, &plan, &gate, &log, &mut runner, &mut sink);
    println!();
    println!("{}", report.render());
    Ok(report)
}
