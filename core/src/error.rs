//! Typed errors for camareLOL.
//!
//! Every failure carries a stable [`ErrorCode`] so the TUI, the Python tools and
//! the Java catalog can all agree on what went wrong without string matching.

use std::fmt;

/// Stable machine readable error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// A required external tool is missing (`lsblk`, `sfdisk`, ...).
    MissingTool,
    /// Running an external tool failed (spawn error).
    SpawnFailed,
    /// External tool exited with a non-zero status.
    CommandFailed,
    /// Output of a discovery tool could not be parsed.
    ParseFailed,
    /// No suitable disk was found for the requested operation.
    NoTargetDisk,
    /// The requested device does not exist any more.
    DeviceGone,
    /// Device identity changed since the plan was created.
    DeviceIdentityChanged,
    /// The device is busy (mounted, in use, cannot be unmounted).
    DeviceBusy,
    /// Plan is internally inconsistent.
    InvalidPlan,
    /// Configuration file failed validation.
    InvalidConfig,
    /// Unsupported schema version of a configuration file.
    UnsupportedSchema,
    /// Boot configuration is not valid for the detected firmware.
    InvalidBootConfig,
    /// Account / hostname / password policy violation.
    InvalidAccount,
    /// Network is unreachable.
    NetworkUnreachable,
    /// A destructive operation was requested while the safety gate is closed.
    SafetyGateClosed,
    /// User has not confirmed a high risk operation.
    ConfirmationRequired,
    /// The update index could not be understood.
    UpdateManifestInvalid,
    /// An update URL was refused because it was not HTTPS.
    UpdateUrlRefused,
    /// A download did not match the checksum published for it.
    UpdateChecksumMismatch,
    /// The update could not be downloaded.
    UpdateDownloadFailed,
    /// The downloaded update would not run; nothing was installed.
    UpdateCandidateFailed,
    /// The running binary cannot be replaced (permissions).
    UpdateNotPrivileged,
    /// The previous binary could not be restored.
    UpdateRollbackFailed,
    /// Feature is explicitly not implemented yet; must never be faked.
    NotSupported,
    /// Generic internal error.
    Internal,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ErrorCode::MissingTool => "missing_tool",
            ErrorCode::SpawnFailed => "spawn_failed",
            ErrorCode::CommandFailed => "command_failed",
            ErrorCode::ParseFailed => "parse_failed",
            ErrorCode::NoTargetDisk => "no_target_disk",
            ErrorCode::DeviceGone => "device_gone",
            ErrorCode::DeviceIdentityChanged => "device_identity_changed",
            ErrorCode::DeviceBusy => "device_busy",
            ErrorCode::InvalidPlan => "invalid_plan",
            ErrorCode::InvalidConfig => "invalid_config",
            ErrorCode::UnsupportedSchema => "unsupported_schema",
            ErrorCode::InvalidBootConfig => "invalid_boot_config",
            ErrorCode::InvalidAccount => "invalid_account",
            ErrorCode::NetworkUnreachable => "network_unreachable",
            ErrorCode::SafetyGateClosed => "safety_gate_closed",
            ErrorCode::ConfirmationRequired => "confirmation_required",
            ErrorCode::UpdateManifestInvalid => "update_manifest_invalid",
            ErrorCode::UpdateUrlRefused => "update_url_refused",
            ErrorCode::UpdateChecksumMismatch => "update_checksum_mismatch",
            ErrorCode::UpdateDownloadFailed => "update_download_failed",
            ErrorCode::UpdateCandidateFailed => "update_candidate_failed",
            ErrorCode::UpdateNotPrivileged => "update_not_privileged",
            ErrorCode::UpdateRollbackFailed => "update_rollback_failed",
            ErrorCode::NotSupported => "not_supported",
            ErrorCode::Internal => "internal",
        };
        f.write_str(s)
    }
}

/// A structured error with a human readable explanation and remediation hints.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CamarelolError {
    pub code: ErrorCode,
    pub message: String,
    /// What the user can do about it (may be empty for trivial cases).
    pub remediation: Option<String>,
    /// Non-sensitive context (command, device path, ...). Secrets are never stored.
    pub context: Option<String>,
}

impl CamarelolError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            remediation: None,
            context: None,
        }
    }

    pub fn with_remediation(mut self, r: impl Into<String>) -> Self {
        self.remediation = Some(r.into());
        self
    }

    pub fn with_context(mut self, c: impl Into<String>) -> Self {
        self.context = Some(c.into());
        self
    }
}

impl fmt::Display for CamarelolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)?;
        if let Some(ctx) = &self.context {
            write!(f, " ({ctx})")?;
        }
        if let Some(rem) = &self.remediation {
            write!(f, " → {rem}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CamarelolError {}

impl From<std::io::Error> for CamarelolError {
    fn from(e: std::io::Error) -> Self {
        CamarelolError::new(ErrorCode::Internal, e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, CamarelolError>;
