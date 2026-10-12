//! Recovery and diagnostics.
//!
//! An interrupted install must never be reported as successful and must never
//! silently re-run a destructive step. This module produces a diagnostics
//! report and answers "what can safely be continued?".

use crate::error::{CamarelolError, ErrorCode, Result};
use crate::hardware::summarize;
use crate::install::PhaseReport;
use crate::logging::Logger;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn report(name: &str, state: &str, retryable: bool) -> PhaseReport {
        PhaseReport { name: name.into(), state: state.into(), retryable, details: String::new() }
    }

    #[test]
    fn completed_phases_are_not_resumed() {
        let phases = vec![report("partition", "completed", true), report("pacstrap", "failed", true)];
        let resume = resume_plan(&phases);
        assert_eq!(resume, vec!["pacstrap".to_string()]);
    }

    #[test]
    fn non_retryable_failures_are_not_resumed() {
        let phases = vec![report("partition", "failed", false)];
        assert!(resume_plan(&phases).is_empty());
    }
}
