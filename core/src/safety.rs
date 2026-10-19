//! The safety layer.
//!
//! Every system-changing request — from the TUI, from Python, or from Java —
//! passes through [`SafetyGate::authorize`]. Destructive work is impossible
//! unless the user explicitly armed the gate **and** typed the confirmation
//! phrase for the risk level of the operation.

use crate::error::{CamarelolError, ErrorCode, Result};
use crate::storage::device::RiskLevel;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Kinds of operations that can destroy data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operation {
    /// Overwrite the partition table of a disk.
    WritePartitionTable,
    /// Create a filesystem on a partition.
    FormatPartition,
    /// Create LUKS container.
    CreateLuks,
    /// Shrink an existing filesystem/partition.
    ResizePartition,
    /// Bootloader installation into the ESP / MBR.
    InstallBootloader,
    /// Run pacstrap (installs packages into the target system).
    Pacstrap,
    /// Mount a partition under the target root (undoable with umount).
    Mount,
}

impl Operation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Operation::WritePartitionTable => "write partition table",
            Operation::FormatPartition => "format partition",
            Operation::CreateLuks => "create LUKS container",
            Operation::ResizePartition => "resize partition",
            Operation::InstallBootloader => "install bootloader",
            Operation::Pacstrap => "install packages into the target system",
            Operation::Mount => "mount a partition into the target system",
        }
    }
}

/// Record of a typed confirmation, kept in memory only (never on disk).
#[derive(Debug, Clone)]
pub struct ConfirmationRecord {
    pub operation: Operation,
    pub phrase: String,
    pub at_unix: u64,
    pub risk: RiskLevel,
}

/// The confirmation phrase the user has to type for a given risk level.
pub fn required_phrase(risk: RiskLevel) -> &'static str {
    match risk {
        RiskLevel::Low => "ERASE",
        RiskLevel::Medium => "ERASE",
        RiskLevel::High => "ERASE AND WIPE",
    }
}

#[derive(Debug, Clone)]
pub struct SafetyGate {
    /// True only after `--allow-destructive` was passed on the command line.
    pub armed: bool,
    /// When true (default) nothing is written; plans are executed in dry-run and
    /// only the commands are recorded. `--execute` sets this to false.
    pub dry_run: bool,
    confirmations: HashMap<(Operation, RiskLevel), ConfirmationRecord>,
}

impl Default for SafetyGate {
    fn default() -> Self {
        SafetyGate {
            armed: false,
            dry_run: true,
            confirmations: HashMap::new(),
        }
    }
}

impl SafetyGate {
    /// Gate that allows writing, still requiring confirmations.
    pub fn armed() -> Self {
        SafetyGate {
            armed: true,
            dry_run: true,
            confirmations: HashMap::new(),
        }
    }

    pub fn armed_and_execute() -> Self {
        SafetyGate {
            armed: true,
            dry_run: false,
            confirmations: HashMap::new(),
        }
    }

    /// Record a typed confirmation. The phrase must match the risk level.
    pub fn confirm(&mut self, op: Operation, risk: RiskLevel, phrase: &str) -> Result<()> {
        // Không phân biệt hoa thường. `erase` và `ERASE` là cùng một ý định:
        // người dùng vẫn phải GÕ ĐÚNG cụm, chỉ là không phải nhớ bật Shift cho
        // từng chữ cái. Khoảng trắng thừa vẫn được bỏ.
        //
        // Ranh giới vẫn phải đúng: `ERASE` không được chấp nhận cho pha cần
        // `ERASE AND WIPE`, và `ERASEE` hay `ERASE X` vẫn phải bị từ chối.
        let wanted = required_phrase(risk);
        if !phrase.trim().eq_ignore_ascii_case(wanted) {
            return Err(CamarelolError::new(
                ErrorCode::ConfirmationRequired,
                format!("confirmation rejected: type exactly `{wanted}` to continue"),
            ));
        }
        let at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.confirmations.insert(
            (op, risk),
            ConfirmationRecord {
                operation: op,
                phrase: phrase.to_string(),
                at_unix: at,
                risk,
            },
        );
        Ok(())
    }

    /// Authorize an operation. This is the single choke point used by every
    /// module that wants to change the system.
    ///
    /// - Locked gate: always refused.
    /// - Dry-run gate: allowed *only* to produce the preview (nothing is
    ///   written by definition), so no confirmation is required yet.
    /// - Execute gate: armed **and** a recorded confirmation for this
    ///   (operation, risk) pair are required.
    pub fn authorize(&self, op: Operation, risk: RiskLevel) -> Result<()> {
        if !self.armed {
            return Err(CamarelolError::new(
                ErrorCode::SafetyGateClosed,
                format!(
                    "refusing to {}: destructive mode is not enabled",
                    op.as_str()
                ),
            )
            .with_remediation(
                "restart camareLOL with --allow-destructive after backing up your data",
            ));
        }
        if self.dry_run {
            return Ok(());
        }
        if !self.confirmations.contains_key(&(op, risk)) {
            return Err(CamarelolError::new(
                ErrorCode::ConfirmationRequired,
                format!(
                    "refusing to {}: not confirmed ({} risk)",
                    op.as_str(),
                    risk.as_str()
                ),
            )
            .with_remediation(format!(
                "type `{}` in the confirmation dialog",
                required_phrase(risk)
            )));
        }
        Ok(())
    }

    /// Convenience: dry-run or execute marker.
    pub fn mode(&self) -> GateMode {
        if !self.armed {
            GateMode::Locked
        } else if self.dry_run {
            GateMode::DryRun
        } else {
            GateMode::Execute
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateMode {
    Locked,
    DryRun,
    Execute,
}

impl GateMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            GateMode::Locked => "locked (no destructive operations possible)",
            GateMode::DryRun => "dry-run (commands are shown, nothing is written)",
            GateMode::Execute => "EXECUTE (destructive operations enabled)",
        }
    }
}

/// What the user must be told before confirming a plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskReport {
    pub device_path: String,
    pub device_identity: String,
    pub risk: RiskLevel,
    pub destructive_operations: Vec<String>,
    pub irreversible: bool,
    pub warnings: Vec<String>,
}

impl RiskReport {
    /// Built from a partition plan (see [`crate::storage::layout::PartitionPlan`]).
    pub fn from_plan(
        device_path: &str,
        device_identity: &str,
        risk: RiskLevel,
        ops: Vec<String>,
    ) -> Self {
        let mut warnings = Vec::new();
        if risk >= RiskLevel::High {
            warnings.push(
                "This device appears to hold another operating system or is used by the live system."
                    .to_string(),
            );
        }
        if risk >= RiskLevel::Medium {
            warnings.push("Existing data on the selected partitions will be lost.".to_string());
        }
        warnings.push("Partition operations cannot always be undone.".to_string());
        RiskReport {
            device_path: device_path.to_string(),
            device_identity: device_identity.to_string(),
            risk,
            destructive_operations: ops,
            irreversible: true,
            warnings,
        }
    }

    pub fn required_phrase(&self) -> &'static str {
        required_phrase(self.risk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locked_gate_refuses_everything() {
        let gate = SafetyGate::default();
        let err = gate
            .authorize(Operation::FormatPartition, RiskLevel::High)
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::SafetyGateClosed);
        assert_eq!(gate.mode(), GateMode::Locked);
    }

    #[test]
    fn dry_run_preview_needs_no_confirmation_but_execute_does() {
        // In dry-run mode the call is allowed because nothing is written: this
        // is what the Review screen shows the user.
        let dry = SafetyGate::armed();
        assert!(dry
            .authorize(Operation::FormatPartition, RiskLevel::High)
            .is_ok());

        // The same call must be refused once the gate is really executing.
        let exec_gate = SafetyGate::armed_and_execute();
        let err = exec_gate
            .authorize(Operation::FormatPartition, RiskLevel::High)
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::ConfirmationRequired);
    }

    #[test]
    fn wrong_phrase_is_rejected() {
        let mut gate = SafetyGate::armed_and_execute();
        assert!(gate
            .confirm(Operation::WritePartitionTable, RiskLevel::Low, "yes")
            .is_err());
        gate.confirm(Operation::WritePartitionTable, RiskLevel::Low, "ERASE")
            .unwrap();
        gate.authorize(Operation::WritePartitionTable, RiskLevel::Low)
            .unwrap();
        // A confirmation for Low does not cover High.
        assert!(gate
            .authorize(Operation::WritePartitionTable, RiskLevel::High)
            .is_err());
    }

    #[test]
    fn high_risk_needs_longer_phrase() {
        assert_eq!(required_phrase(RiskLevel::High), "ERASE AND WIPE");
    }
}

#[cfg(test)]
mod phrase_case_tests {
    use super::*;

    fn armed() -> SafetyGate {
        SafetyGate {
            armed: true,
            dry_run: false,
            ..Default::default()
        }
    }

    #[test]
    fn confirmation_ignores_case_but_not_content() {
        let mut g = armed();
        for typed in ["ERASE", "erase", "Erase", "eRaSe", "  ERASE  "] {
            assert!(
                g.confirm(Operation::WritePartitionTable, RiskLevel::Low, typed)
                    .is_ok(),
                "phải nhận `{typed}`"
            );
        }
    }

    #[test]
    fn high_risk_still_needs_both_words() {
        let mut g = armed();
        assert!(
            g.confirm(Operation::FormatPartition, RiskLevel::High, "ERASE")
                .is_err(),
            "ERASE một mình không đủ cho pha high-risk"
        );
        assert!(g
            .confirm(
                Operation::FormatPartition,
                RiskLevel::High,
                "erase and wipe"
            )
            .is_ok());
        // và phải thực sự khớp, không phải chỉ chứa
        for wrong in ["ERASEE", "ERASE X", "WIPE", "ERASE AND WIP"] {
            assert!(
                g.confirm(Operation::FormatPartition, RiskLevel::High, wrong)
                    .is_err(),
                "`{wrong}` phải bị từ chối"
            );
        }
    }
}
