//! Installation orchestration: phase states, preconditions and reporting.

pub mod execute;
pub mod phases;

pub use execute::run_install;

use crate::config::InstallationConfig;
use crate::hardware::Firmware;
use crate::storage::layout::PartitionPlan;
use serde::{Deserialize, Serialize};

/// Explicit phase states required by the specification.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PhaseState {
    Pending,
    Running,
    Completed,
    Warning,
    Failed,
    Skipped,
}

impl PhaseState {
    pub fn as_str(&self) -> &'static str {
        match self {
            PhaseState::Pending => "pending",
            PhaseState::Running => "running",
            PhaseState::Completed => "completed",
            PhaseState::Warning => "warning",
            PhaseState::Failed => "failed",
            PhaseState::Skipped => "skipped",
        }
    }
}

/// Serializable outcome of one phase (kept for logs and recovery).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseReport {
    pub name: String,
    pub state: String,
    /// Whether the phase may be re-run after a failure without being unsafe.
    pub retryable: bool,
    pub details: String,
}

/// Overall verdict after the run.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Outcome {
    Completed,
    CompletedWithWarnings,
    Incomplete,
    BootNotVerifiable,
}

impl Outcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Outcome::Completed => "completed",
            Outcome::CompletedWithWarnings => "completed with warnings",
            Outcome::Incomplete => "incomplete",
            Outcome::BootNotVerifiable => "boot cannot be verified",
        }
    }
}

/// Everything a phase needs to check its preconditions.
pub struct Preflight {
    pub firmware: Firmware,
    pub plan: PartitionPlan,
    pub config: InstallationConfig,
}

/// A step executed inside the target system.
#[derive(Debug, Clone)]
pub struct ChrootStep {
    pub description: String,
    /// Written to the target's /tmp and executed with bash. Passwords may only
    /// appear inside a script that lives in memory and is piped on stdin.
    pub script: String,
    pub retryable: bool,
}

/// Progress events for the TUI.
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    PhaseStarted { name: String },
    PhaseFinished { name: String, state: PhaseState },
    CommandPreview { command: String },
    Info { message: String },
}

pub trait ProgressSink {
    fn emit(&mut self, event: ProgressEvent);
}

/// No-op sink for tests.
pub struct NullSink;
impl ProgressSink for NullSink {
    fn emit(&mut self, _event: ProgressEvent) {}
}

/// Partition path for the n-th partition of `disk` (1-based).
pub fn partition_path(disk: &str, n: usize) -> String {
    if disk.contains("nvme") || disk.contains("mmcblk") {
        format!("{disk}p{n}")
    } else {
        format!("{disk}{n}")
    }
}

/// The complete list of phases, in execution order.
pub const PHASE_ORDER: &[&str] = &[
    "preflight",
    "partition",
    "format",
    "mount",
    "pacstrap",
    "fstab",
    "bootloader",
    "system-config",
    "accounts",
    "services",
    "initramfs",
    "post-check",
];

/// Human readable description for the progress screen.
pub fn describe_phase(name: &str) -> &'static str {
    match name {
        "preflight" => "Re-checking firmware and validating the plan",
        "partition" => "Writing the partition table and partitions",
        "format" => "Creating filesystems",
        "mount" => "Mounting the target system",
        "pacstrap" => "Installing the base system",
        "fstab" => "Generating fstab",
        "bootloader" => "Installing the bootloader",
        "system-config" => "Writing locale, timezone and hostname",
        "accounts" => "Creating users and setting passwords",
        "services" => "Enabling services",
        "initramfs" => "Generating the initramfs",
        "post-check" => "Verifying the installed system",
        _ => "Working",
    }
}

/// Bootloader enum re-exported for convenience of the UI layer.
pub use crate::boot::Bootloader as InstallBootloader;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_path_handles_nvme() {
        assert_eq!(partition_path("/dev/vda", 2), "/dev/vda2");
        assert_eq!(partition_path("/dev/nvme0n1", 2), "/dev/nvme0n1p2");
        assert_eq!(partition_path("/dev/mmcblk0", 1), "/dev/mmcblk0p1");
    }

    #[test]
    fn every_phase_has_a_description() {
        for p in PHASE_ORDER {
            assert!(!describe_phase(p).is_empty());
        }
    }
}
