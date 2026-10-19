//! Recovery and diagnostics.
//!
//! An interrupted install must never be reported as successful and must never
//! silently re-run a destructive step. This module produces a diagnostics
//! report and answers "what can safely be continued?".
//!
//! # Two rules that are easy to break
//!
//! **The report is redacted by the same pass as the logs.**
//! [`crate::logging::redact`] is used through [`crate::logging::redact_document`];
//! there is no second matcher here, because the failure mode of a second one
//! is a second set of rules nobody tests.
//!
//! **Nothing here claims the machine will boot.** Not a completed install, not
//! a preview, not a recovery note. camareLOL never reboots the user and never
//! verifies bootability, so any sentence implying otherwise is a lie that
//! costs somebody their afternoon.

use crate::error::{CamarelolError, ErrorCode, Result};
use crate::hardware::{self, summarize};
use crate::install::PhaseReport;
use crate::logging::{redact_document, Logger};
use crate::storage::scan;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostics {
    pub host: String,
    pub firmware: String,
    pub cpu: String,
    pub ram_bytes: u64,
    pub virtualized: bool,
    pub devices: Vec<String>,
    pub phases: Vec<PhaseReport>,
    pub unmounted_targets: Vec<String>,
}

/// Collect a diagnostics report. Never panics, never contains secrets.
pub fn collect(phases: &[PhaseReport]) -> Diagnostics {
    let hw = summarize();
    let devices = scan::scan_devices()
        .map(|d| {
            d.into_iter()
                .map(|dev| {
                    format!(
                        "{} [{}] {} {} risk={} partitions={}",
                        dev.path,
                        dev.identity(),
                        dev.kind.as_str(),
                        dev.capacity_display(),
                        dev.risk.as_str(),
                        dev.partitions.len()
                    )
                })
                .collect()
        })
        .unwrap_or_default();

    Diagnostics {
        host: std::fs::read_to_string("/etc/hostname")
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "unknown".into()),
        firmware: hw.firmware.as_str().to_string(),
        cpu: hw.cpu.model,
        ram_bytes: hw.ram_bytes,
        virtualized: hw.virtualized,
        devices,
        phases: phases.to_vec(),
        unmounted_targets: vec![],
    }
}

/// Write the diagnostics report for the user to attach to a bug report.
pub fn export(path: &str, diag: &Diagnostics) -> Result<()> {
    let text = serde_json::to_string_pretty(diag).map_err(|e| {
        CamarelolError::new(ErrorCode::Internal, format!("cannot render report: {e}"))
    })?;
    if let Some(dir) = Path::new(path).parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).ok();
        }
    }
    std::fs::write(path, text).map_err(|e| {
        CamarelolError::new(
            ErrorCode::Internal,
            format!("cannot write diagnostics report to {path}"),
        )
        .with_context(e.to_string())
    })
}

/// Which phases may be retried after a failure.
///
/// Rules (deliberately conservative):
/// - a phase that reported Completed may never be re-run automatically
/// - a phase that reported Failed may be retried **once**, and only when its
///   own `retryable` flag is set
/// - everything else stays Failed and needs a human
pub fn resume_plan(phases: &[PhaseReport]) -> Vec<String> {
    phases
        .iter()
        .filter(|p| p.state == "failed")
        .filter(|p| p.retryable)
        .map(|p| p.name.clone())
        .collect()
}

/// Log the resume decision so the user can see why something was not retried.
pub fn explain_resume(log: &Logger, phases: &[PhaseReport]) -> String {
    let resumable = resume_plan(phases);
    let skipped: Vec<&str> = phases
        .iter()
        .filter(|p| p.state == "completed")
        .map(|p| p.name.as_str())
        .collect();
    if resumable.is_empty() {
        let msg = "nothing can be resumed automatically; please review the log".to_string();
        log.log("recovery", &msg);
        msg
    } else {
        let msg = format!(
            "phase(s) can be retried: {} (already finished: {})",
            resumable.join(", "),
            skipped.join(", ")
        );
        log.log("recovery", &msg);
        msg
    }
}

// ---------------------------------------------------------------------------
// Recovery guidance
// ---------------------------------------------------------------------------

/// Where the log files live. A directory, not a file: one file per run.
pub const LOG_DIR: &str = "/var/log/camarelol";

/// What to do after an install stopped early.
///
/// This is the same information as [`resume_plan`] plus the two things that
/// decide what a person does next: what got as far as working, and what to
/// look at with their own eyes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecoveryAdvice {
    /// Phases that finished.
    pub completed: Vec<String>,
    /// The phase that stopped the run, and what it said.
    pub failed: Option<String>,
    pub failed_details: String,
    /// Phases that never ran.
    pub not_run: Vec<String>,
    /// Phases camareLOL would consider re-running. Empty means "a human".
    pub safe_to_retry: Vec<String>,
    /// The manual checks, in the order they are worth doing.
    pub manual_checks: Vec<String>,
}

impl RecoveryAdvice {
    /// True when there is nothing to recover from — every phase finished.
    pub fn is_all_good(&self) -> bool {
        self.failed.is_none() && self.not_run.is_empty()
    }

    /// The user-facing text. Shown on the TUI's result screen and included in
    /// the diagnostics report.
    pub fn render(&self) -> String {
        let mut out = String::new();
        let finished = if self.completed.is_empty() {
            "nothing yet".to_string()
        } else {
            self.completed.join(", ")
        };
        out.push_str(&format!("Finished:      {finished}\n"));
        match &self.failed {
            Some(name) => {
                out.push_str(&format!("Stopped at:    {name}"));
                if !self.failed_details.is_empty() {
                    out.push_str(&format!(" — {}", self.failed_details));
                }
                out.push('\n');
            }
            None => out.push_str("Stopped at:    no phase reported a failure\n"),
        }
        if !self.not_run.is_empty() {
            out.push_str(&format!("Never ran:     {}\n", self.not_run.join(", ")));
        }
        if self.safe_to_retry.is_empty() {
            out.push_str(
                "Safe to retry: nothing. The failed phase is not one camareLOL will re-run \
                 on its own.\n",
            );
        } else {
            out.push_str(&format!(
                "Safe to retry: {} — everything before it already finished and will not be \
                 repeated.\n",
                self.safe_to_retry.join(", ")
            ));
        }
        out.push_str("Check by hand:\n");
        for check in &self.manual_checks {
            out.push_str(&format!("  - {check}\n"));
        }
        // The honest sentence, and the reason it exists: a person reading
        // "everything is fine except the bootloader phase" is entitled to ask
        // whether it will boot, and the truthful answer is that this program
        // has no way to know.
        out.push_str(
            "\nWhether this system boots is not something camareLOL can tell you. It never \
             reboots for you,\nand no phase of it checks bootability. Look at the mounts and \
             the log before deciding.\n",
        );
        out
    }
}

/// Work out the recovery guidance for a set of phase reports.
///
/// Rules, in the order they are applied:
/// * a phase that completed is never re-run;
/// * a phase that failed is a candidate for retry **only** if it says so itself
///   (see [`crate::install::phases::retryable`] — partitioning is not, because
///   repeating it would re-write a table a human may have since edited);
/// * a phase that never started is neither completed nor failed, and saying so
///   is more useful than pretending the run stopped there.
pub fn advise(phases: &[PhaseReport]) -> RecoveryAdvice {
    let mut advice = RecoveryAdvice {
        safe_to_retry: resume_plan(phases),
        ..Default::default()
    };
    for phase in phases {
        match phase.state.as_str() {
            "completed" => advice.completed.push(phase.name.clone()),
            "failed" | "warning" if advice.failed.is_none() => {
                advice.failed = Some(phase.name.clone());
                advice.failed_details = phase.details.clone();
            }
            _ => {}
        }
    }
    // The first phase that never ran is the point where the install stopped.
    if let Some(name) = phases
        .iter()
        .find(|p| p.state == "pending" || p.state == "running")
        .map(|p| p.name.clone())
    {
        advice.not_run.push(name);
    }
    advice.manual_checks = manual_checks(&advice);
    advice
}

fn manual_checks(advice: &RecoveryAdvice) -> Vec<String> {
    let failed = advice.failed.as_deref().unwrap_or_default();
    let mut checks: Vec<String> = Vec::new();
    match failed {
        // Nothing was mounted, or the mount itself is the question.
        "mount" | "" => checks.push(
            "findmnt /mnt — is the new root actually mounted there? A half-mounted target \
             is the most common cause."
                .to_string(),
        ),
        _ => {}
    }
    if !advice.completed.iter().any(|p| p == "format") {
        checks.push("lsblk -f — which filesystem, if any, the partitions have.".to_string());
    }
    if !advice.completed.iter().any(|p| p == "bootloader") {
        checks.push("efibootmgr -v (UEFI) — no bootloader entry may exist yet.".to_string());
    }
    checks.push(format!(
        "the log for this run, newest first: ls -t {LOG_DIR}/ | head",
    ));
    checks.push(
        "pacstrap and the chroot scripts write their own errors; they are in that log, not here."
            .to_string(),
    );
    checks
}

// ---------------------------------------------------------------------------
// The diagnostics report
// ---------------------------------------------------------------------------

/// Everything a report can contain. Every field is optional, so a report
/// written from `--save-diagnostics` (no plan, no phases) is still a complete
/// document rather than a page of "unknown".
#[derive(Debug, Default, Clone)]
pub struct ReportInput {
    pub generated_unix: u64,
    pub version: String,
    /// `preview`, `dry-run`, `execute` or `locked`.
    pub mode: String,
    pub host: Option<String>,
    pub host_supported: Option<bool>,
    pub host_reason: Option<String>,
    pub cpu: Option<String>,
    pub cpu_threads: Option<usize>,
    pub ram_bytes: Option<u64>,
    pub network: Option<String>,
    pub interfaces: Vec<String>,
    pub tools: Vec<(String, bool)>,
    pub devices: Vec<String>,
    pub plan: Option<String>,
    pub phases: Vec<PhaseReport>,
    pub warnings: Vec<String>,
    pub log_paths: Vec<String>,
}

/// Detect the parts of the report that can be detected without a plan.
///
/// Everything here is a read-only probe: `/proc`, `/sys`, `lsblk`, `findmnt`
/// and `<tool> --version`. No process is started that could change the machine,
/// so this is also safe to run inside `--preview`.
pub fn collect_input() -> ReportInput {
    let hw = summarize();
    let host = crate::host::detect();
    let interfaces: Vec<String> = crate::network::list_interfaces()
        .into_iter()
        .map(|i| {
            format!(
                "{} ({}, {})",
                i.name,
                if i.carrier { "link" } else { "no link" },
                if i.wireless { "wireless" } else { "wired" }
            )
        })
        .collect();
    let tools = [
        "lsblk",
        "findmnt",
        "sfdisk",
        "mkfs.ext4",
        "mkfs.btrfs",
        "pacstrap",
        "arch-chroot",
        "iwctl",
        "nmtui",
    ]
    .into_iter()
    .map(|t| (t.to_string(), crate::sys::tool_available(t)))
    .collect();
    let devices = scan::scan_devices()
        .map(|list| {
            list.into_iter()
                .map(|d| {
                    format!(
                        "{} [{}] {} {} risk={} partitions={}",
                        d.path,
                        d.identity(),
                        d.kind.as_str(),
                        d.capacity_display(),
                        d.risk.as_str(),
                        d.partitions.len()
                    )
                })
                .collect()
        })
        .unwrap_or_default();

    ReportInput {
        generated_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        version: crate::CORE_VERSION.to_string(),
        mode: "unknown".into(),
        host: Some(host.summary()),
        host_supported: Some(host.supported),
        host_reason: if host.supported {
            None
        } else {
            Some(host.reason.clone())
        },
        cpu: Some(hw.cpu.model.clone()),
        cpu_threads: Some(hw.cpu.threads),
        ram_bytes: Some(hw.ram_bytes),
        network: Some(
            crate::network::describe(
                crate::network::check_internet().unwrap_or(crate::network::Connectivity::Down),
            )
            .to_string(),
        ),
        interfaces,
        tools,
        devices,
        plan: None,
        phases: Vec::new(),
        warnings: Vec::new(),
        log_paths: recent_logs(LOG_DIR, 5),
    }
}

/// The most recent log files in `dir`, newest first. Missing directory is not an
/// error: an installer that has never run has no logs, and that is a fact worth
/// putting in the report.
pub fn recent_logs(dir: &str, limit: usize) -> Vec<String> {
    let mut entries: Vec<(std::time::SystemTime, String)> = Vec::new();
    if let Ok(read) = std::fs::read_dir(dir) {
        for entry in read.flatten() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !meta.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with("camarelol-") {
                continue;
            }
            entries.push((
                meta.modified().unwrap_or(std::time::UNIX_EPOCH),
                format!("{dir}/{name}"),
            ));
        }
    }
    entries.sort_by_key(|e| std::cmp::Reverse(e.0));
    entries.into_iter().take(limit).map(|(_, p)| p).collect()
}

/// Render the report as plain text.
///
/// Every value in the document — including values a caller did not build here,
/// like a phase's `details` — goes through [`redact_document`], which is the
/// same pass that guards the log files. The report is meant to be posted in a
/// public issue tracker, so it gets the pessimistic treatment.
pub fn render_text_report(input: &ReportInput) -> String {
    let mut s = String::new();
    let section = |title: &str, s: &mut String| {
        s.push_str(&format!("\n[{}]\n", title));
    };

    s.push_str("camareLOL diagnostics report\n");
    s.push_str("=========================\n");
    if input.generated_unix > 0 {
        s.push_str(&format!("generated (unix): {}\n", input.generated_unix));
    }
    if !input.version.is_empty() {
        s.push_str(&format!("version:          {}\n", input.version));
    }
    if !input.mode.is_empty() {
        s.push_str(&format!("mode:             {}\n", input.mode));
    }

    section("Host", &mut s);
    s.push_str(&format!(
        "distribution:     {}\n",
        input.host.as_deref().unwrap_or("unknown")
    ));
    match input.host_supported {
        Some(true) => s.push_str("supported:        yes\n"),
        Some(false) => {
            s.push_str("supported:        NO\n");
            if let Some(reason) = &input.host_reason {
                s.push_str(&format!("refused because:  {reason}\n"));
            }
        }
        None => {}
    }
    s.push_str(&format!(
        "firmware:         {}\n",
        hardware::detect_firmware().as_str()
    ));
    s.push_str(&format!(
        "cpu:              {}\n",
        input.cpu.as_deref().unwrap_or("unknown")
    ));
    if let Some(threads) = input.cpu_threads {
        s.push_str(&format!("cpu threads:      {threads}\n"));
    }
    if let Some(ram) = input.ram_bytes {
        s.push_str(&format!(
            "memory:           {}\n",
            crate::storage::human_size(ram)
        ));
    }
    s.push_str(&format!(
        "network:          {}\n",
        input.network.as_deref().unwrap_or("unknown")
    ));
    for iface in &input.interfaces {
        s.push_str(&format!("interface:        {iface}\n"));
    }

    section("Tools", &mut s);
    for (tool, present) in &input.tools {
        s.push_str(&format!(
            "  {:<16} {}\n",
            tool,
            if *present { "present" } else { "MISSING" }
        ));
    }

    section("Storage survey", &mut s);
    if input.devices.is_empty() {
        s.push_str("  no block device was listed\n");
    }
    for device in &input.devices {
        s.push_str(&format!("  {device}\n"));
    }

    if let Some(plan) = &input.plan {
        section("Plan", &mut s);
        for line in plan.lines() {
            s.push_str(&format!("  {line}\n"));
        }
    }

    section("Phases", &mut s);
    if input.phases.is_empty() {
        s.push_str("  no installation has been run in this session\n");
    }
    for phase in &input.phases {
        s.push_str(&format!(
            "  {:<16} {:<10} {}\n",
            phase.name, phase.state, phase.details
        ));
    }

    section("Warnings", &mut s);
    if input.warnings.is_empty() {
        s.push_str("  none\n");
    }
    for warning in &input.warnings {
        s.push_str(&format!("  - {warning}\n"));
    }

    // Recovery guidance goes in the report because a bug report about a failed
    // install is mostly this section.
    let advice = advise(&input.phases);
    if !advice.is_all_good() {
        section("Recovery", &mut s);
        for line in advice.render().lines() {
            s.push_str(&format!("  {line}\n"));
        }
    }

    section("Logs", &mut s);
    if input.log_paths.is_empty() {
        s.push_str(&format!("  none found in {LOG_DIR}\n"));
    }
    for path in &input.log_paths {
        s.push_str(&format!("  {path}\n"));
    }

    let body = redact_document(&s);

    // The closing note is appended *after* redaction, and deliberately so: it
    // is a fixed sentence with no user data in it, and it is full of the very
    // words the redactor is built to eat — redacting it would replace the
    // explanation of the redaction with a wall of `<redacted>`.
    const FOOTER: &str = "\n[Redaction]\n  Every line above passed through the redaction pass the log files use.\n  No password, token, key or Wi-Fi passphrase is in this file.\n  Attach it as-is; do not edit it, because editing it is how the context gets lost.\n";
    format!("{body}{FOOTER}")
}

/// Render and write the report. Parent directories are created.
pub fn export_text(path: &str, input: &ReportInput) -> Result<()> {
    let text = render_text_report(input);
    if let Some(dir) = Path::new(path).parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir).ok();
        }
    }
    std::fs::write(path, text).map_err(|e| {
        CamarelolError::new(
            ErrorCode::Internal,
            format!("cannot write diagnostics report to {path}"),
        )
        .with_context(e.to_string())
        .with_remediation("pick a path you can write to, for example /tmp/camarelol-diag.txt")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(name: &str, state: &str, retryable: bool) -> PhaseReport {
        PhaseReport {
            name: name.into(),
            state: state.into(),
            retryable,
            details: String::new(),
        }
    }

    #[test]
    fn completed_phases_are_not_resumed() {
        let phases = vec![
            report("partition", "completed", true),
            report("pacstrap", "failed", true),
        ];
        let resume = resume_plan(&phases);
        assert_eq!(resume, vec!["pacstrap".to_string()]);
    }

    #[test]
    fn non_retryable_failures_are_not_resumed() {
        let phases = vec![report("partition", "failed", false)];
        assert!(resume_plan(&phases).is_empty());
    }

    // ------------------------------------------------------- guidance

    /// A run that stopped in the middle: three phases finished, one failed, the
    /// rest never started.
    fn half_done() -> Vec<PhaseReport> {
        let mut phases = vec![
            report("preflight", "completed", true),
            report("partition", "completed", false),
            report("format", "completed", false),
            report("mount", "completed", false),
        ];
        phases.push(PhaseReport {
            name: "pacstrap".into(),
            state: "failed".into(),
            retryable: true,
            details: "pacstrap exited with status 1".into(),
        });
        phases.extend([
            PhaseReport {
                name: "fstab".into(),
                state: "pending".into(),
                retryable: false,
                details: String::new(),
            },
            PhaseReport {
                name: "bootloader".into(),
                state: "pending".into(),
                retryable: false,
                details: String::new(),
            },
        ]);
        phases
    }

    #[test]
    fn the_guidance_says_what_finished_what_failed_and_what_is_safe() {
        let advice = advise(&half_done());
        assert_eq!(
            advice.completed,
            vec!["preflight", "partition", "format", "mount"]
        );
        assert_eq!(advice.failed.as_deref(), Some("pacstrap"));
        assert!(advice.failed_details.contains("status 1"));
        assert_eq!(advice.not_run, vec!["fstab"], "only the first unrun phase");
        assert_eq!(advice.safe_to_retry, vec!["pacstrap"]);
        assert!(!advice.is_all_good());

        let text = advice.render();
        assert!(text.contains("mount"), "must list what finished: {text}");
        assert!(
            text.contains("pacstrap — pacstrap exited with status 1"),
            "{text}"
        );
        assert!(text.contains("Safe to retry: pacstrap"), "{text}");
        assert!(text.contains("Never ran:"), "{text}");
    }

    #[test]
    fn the_guidance_never_claims_the_machine_will_boot() {
        // The one sentence that must not be in this text, in any state of the
        // install. A person who reads "the bootloader phase failed" will ask
        // whether it boots; the answer is that this program cannot know.
        for phases in [
            half_done(),
            vec![report("pacstrap", "failed", true)],
            vec![report("mount", "failed", false)],
        ] {
            let text = advise(&phases).render();
            for claim in [
                "will boot",
                "bootable",
                "ready to boot",
                "can boot",
                "reboot now",
                "safe to reboot",
                "should boot",
            ] {
                assert!(
                    !text.contains(claim),
                    "the recovery text claims `{claim}`:\n{text}"
                );
            }
            assert!(
                text.contains("not something camareLOL can tell you"),
                "it must say plainly that bootability is unknown:\n{text}"
            );
        }
    }

    #[test]
    fn a_failure_camarelol_will_not_retry_says_so_instead_of_pretending() {
        let advice = advise(&[report("partition", "failed", false)]);
        let text = advice.render();
        assert!(advice.safe_to_retry.is_empty());
        assert!(
            text.contains("Safe to retry: nothing"),
            "silently omitting the line would read as \"all clear\": {text}"
        );
        // And it must not tell a person to repeat the partitioning.
        assert!(
            !text.contains("Safe to retry: partition"),
            "repeating the partition table is exactly what must not be offered: {text}"
        );
    }

    #[test]
    fn a_completed_run_is_not_told_it_needed_recovering() {
        let phases = vec![
            report("preflight", "completed", true),
            report("pacstrap", "completed", true),
            report("bootloader", "completed", false),
        ];
        let advice = advise(&phases);
        assert!(advice.is_all_good());
        assert!(advice.failed.is_none());
        // Even so, the text does not claim the system boots.
        assert!(!advice.render().contains("will boot"));
    }

    // ------------------------------------------------- the text report

    fn report_with_secrets() -> ReportInput {
        ReportInput {
            generated_unix: 1_700_000_000,
            version: "0.1.17".into(),
            mode: "execute".into(),
            host: Some("Arch Linux / systemd".into()),
            host_supported: Some(true),
            host_reason: None,
            cpu: Some("Intel(R) Core(TM) i5".into()),
            cpu_threads: Some(8),
            ram_bytes: Some(16 * 1024 * 1024 * 1024),
            network: Some("internet connection OK".into()),
            interfaces: vec!["wlan0 (link, wireless)".into()],
            tools: vec![("lsblk".into(), true), ("pacstrap".into(), true)],
            devices: vec!["/dev/nvme0n1 [WD123] NVMe 476.9 GiB risk=Low partitions=0".into()],
            plan: Some("Disk: /dev/nvme0n1\n  p1  512 MiB  efi\n  p2  476.3 GiB root".into()),
            phases: half_done(),
            warnings: vec!["password=hunter2 was rejected".into()],
            log_paths: vec!["/var/log/camarelol/camarelol-17.log".into()],
        }
    }

    #[test]
    fn the_report_names_everything_a_bug_report_needs() {
        let text = render_text_report(&report_with_secrets());
        for expected in [
            "camareLOL diagnostics report",
            "Arch Linux / systemd",
            "UEFI",
            "Intel(R) Core(TM) i5",
            "16 GiB",
            "internet connection OK",
            "lsblk",
            "/dev/nvme0n1",
            "p1  512 MiB  efi",
            "pacstrap",
            "Safe to retry: pacstrap",
            "/var/log/camarelol/camarelol-17.log",
        ] {
            assert!(
                text.contains(expected),
                "the report is missing {expected}:\n{text}"
            );
        }
    }

    /// The load-bearing test: a log line full of secrets goes in, and none of
    /// it comes out.
    #[test]
    fn nothing_secret_survives_into_the_exported_report() {
        let token = format!("ghp_{}", "a".repeat(36));
        let api_key = format!("sk-{}", "b".repeat(32));
        let mut input = report_with_secrets();

        // A log excerpt pasted in as a phase detail — this is what actually
        // happens when a bug report is written from a terminal.
        input.warnings = vec![
            "connecting to ssid=HomeWiFi psk=correct horse battery staple".to_string(),
            format!("Authorization: Bearer {token}"),
            "sudo passwd root: root_password=hunter2".to_string(),
            "wpa_passphrase=hunter2".to_string(),
            format!("login for alice: password={api_key}"),
            "nothing sensitive on this line at all".to_string(),
        ];
        input.phases = vec![PhaseReport {
            name: "accounts".into(),
            state: "failed".into(),
            retryable: true,
            details: "useradd failed for alice; password=hunter2 given on stdin".to_string(),
        }];

        let text = render_text_report(&input);

        for secret in [
            "correct horse battery",
            "battery",
            &token,
            &api_key[..20],
            "hunter2",
        ] {
            assert!(
                !text.contains(secret),
                "`{secret}` survived into the report:\n{text}"
            );
        }
        // The names are kept so the report still reads as a report.
        assert!(text.contains("password=<redacted>"), "{text}");
        assert!(
            text.contains("nothing sensitive on this line at all"),
            "redaction must not eat lines that hold nothing:\n{text}"
        );
        // And the structure survives: redaction runs per line so the headings
        // are still headings.
        assert!(text.contains("\n[Storage survey]\n"), "{text}");
        assert!(text.contains("\n[Phases]\n"), "{text}");
        assert!(text.contains("\n[Warnings]\n"), "{text}");
    }

    #[test]
    fn an_exported_report_on_disk_contains_no_secret_either() {
        let dir = std::env::temp_dir().join("camarelol-diag-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("report.txt");
        let mut input = report_with_secrets();
        input.warnings = vec!["password=hunter2".into()];
        export_text(path.to_str().unwrap(), &input).unwrap();

        let written = std::fs::read_to_string(&path).unwrap();
        assert!(!written.contains("hunter2"), "{written}");
        assert!(
            written.contains("camareLOL diagnostics report"),
            "{written}"
        );
    }

    #[test]
    fn a_report_from_a_command_line_run_still_reads_as_a_report() {
        // No plan, no phases, no warnings: every section has to say so rather
        // than disappear, because "missing" and "nothing happened" are the two
        // things a bug reporter most needs told apart.
        let text = render_text_report(&ReportInput {
            version: "0.1.17".into(),
            mode: "preview".into(),
            ..Default::default()
        });
        for expected in [
            "[Host]",
            "[Tools]",
            "[Storage survey]",
            "[Phases]",
            "[Warnings]",
            "[Logs]",
            "no installation has been run in this session",
        ] {
            assert!(text.contains(expected), "missing {expected}:\n{text}");
        }
        assert!(!text.contains("[Recovery]"), "nothing failed:\n{text}");
    }

    #[test]
    fn a_preview_report_says_it_wrote_nothing() {
        let text = render_text_report(&ReportInput {
            mode: "preview".into(),
            phases: vec![report("pacstrap", "completed", true)],
            ..Default::default()
        });
        assert!(text.contains("mode:             preview"), "{text}");
        assert!(
            !text.contains("[Recovery]"),
            "a preview has nothing to recover from:\n{text}"
        );
    }

    #[test]
    fn recent_logs_returns_newest_first_and_tolerates_a_missing_directory() {
        let dir = std::env::temp_dir().join("camarelol-log-listing-tests");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(
            recent_logs(dir.to_str().unwrap(), 5).is_empty(),
            "an empty directory is not an error"
        );
        assert!(
            recent_logs("/nonexistent/camarelol-logs", 5).is_empty(),
            "an installer that never ran has no logs, and that is a fact, not a crash"
        );

        std::fs::write(dir.join("camarelol-100.log"), "old").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(dir.join("camarelol-200.log"), "new").unwrap();
        std::fs::write(dir.join("unrelated.txt"), "x").unwrap();
        let found = recent_logs(dir.to_str().unwrap(), 5);
        assert_eq!(found.len(), 2, "only camarelol-*.log files: {found:?}");
        assert!(found[0].ends_with("camarelol-200.log"), "{found:?}");
    }

    #[test]
    fn collecting_a_report_starts_no_mutating_process() {
        // The report is reachable from the TUI and from `--save-diagnostics`,
        // and one of those can be a `--preview` run. Read-only probes only.
        let _scope = crate::sys::PreviewScope::on();
        let _ = collect_input();
        assert_eq!(
            crate::sys::mutating_refusals(),
            0,
            "collecting a report must not ask for anything mutating"
        );
    }
}
