//! Versioned JSON IPC protocol between the Rust core, the Python tools and the
//! Java catalog.
//!
//! Design rules:
//! - every request has an `id`; responses echo it
//! - every response carries the `protocol_version` it was produced with
//! - errors are typed ([`ErrorCode`]), never plain strings
//! - timeouts and cancellation are part of the request, not an afterthought
//! - the Rust core validates **every** request, so Python/Java can never bypass
//!   the safety layer
//!
//! The wire format is newline-delimited JSON: one [`Request`] per line, one
//! [`Response`] per line. See `docs/IPC-PROTOCOL.md` for the full spec.

use crate::error::ErrorCode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u32 = 1;

/// Default per-request timeout in milliseconds.
pub const DEFAULT_TIMEOUT_MS: u64 = 120_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Monotonic per-connection request id.
    pub id: u64,
    pub protocol_version: u32,
    pub method: Method,
    pub params: Value,
    /// Per-request timeout; `None` means [`DEFAULT_TIMEOUT_MS`].
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// Client may set this to request cancellation of an earlier id.
    #[serde(default)]
    pub cancel: Option<u64>,
}

impl Request {
    pub fn new(id: u64, method: Method, params: Value) -> Self {
        Request {
            id,
            protocol_version: PROTOCOL_VERSION,
            method,
            params,
            timeout_ms: None,
            cancel: None,
        }
    }

    pub fn timeout(&self) -> u64 {
        self.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    /// Get a full survey of storage devices.
    ScanDevices,
    /// Build a partition plan for a disk.
    CreatePlan,
    /// Validate an existing plan.
    ValidatePlan,
    /// **Destructive**: write the partition table.
    ApplyPlan,
    /// Format planned partitions.
    FormatPartitions,
    /// Health/diagnostic report.
    Diagnostics,
    /// Read the current configuration.
    GetConfig,
    /// **Validated** configuration update.
    SetConfig,
    /// Ask the Python/Java side for package recommendations (informational).
    RecommendPackages,
    /// Ping with protocol discovery.
    Hello,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Ok,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl From<crate::error::CamarelolError> for IpcError {
    fn from(e: crate::error::CamarelolError) -> Self {
        IpcError {
            code: e.code,
            message: e.message,
            remediation: e.remediation,
            details: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub id: u64,
    pub protocol_version: u32,
    pub status: ResponseStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<IpcError>,
    /// How long the call took, in milliseconds.
    pub duration_ms: u128,
}

impl Response {
    pub fn ok(id: u64, result: Value, took: u128) -> Self {
        Response {
            id,
            protocol_version: PROTOCOL_VERSION,
            status: ResponseStatus::Ok,
            result: Some(result),
            error: None,
            duration_ms: took,
        }
    }

    pub fn err(id: u64, e: IpcError, took: u128) -> Self {
        Response {
            id,
            protocol_version: PROTOCOL_VERSION,
            status: ResponseStatus::Error,
            result: None,
            error: Some(e),
            duration_ms: took,
        }
    }

    /// Serialize as one wire line (with trailing newline for NDJSON).
    pub fn to_line(&self) -> Result<String, serde_json::Error> {
        let mut s = serde_json::to_string(self)?;
        s.push('\n');
        Ok(s)
    }
}

/// Decode one wire line.
pub fn parse_request(line: &str) -> Result<Request, IpcError> {
    let req: Request = serde_json::from_str(line).map_err(|e| IpcError {
        code: ErrorCode::ParseFailed,
        message: format!("cannot parse request: {e}"),
        remediation: Some("send one JSON object per line".into()),
        details: None,
    })?;
    if req.protocol_version != PROTOCOL_VERSION {
        return Err(IpcError {
            code: ErrorCode::UnsupportedSchema,
            message: format!(
                "unsupported protocol version {} (core speaks {})",
                req.protocol_version, PROTOCOL_VERSION
            ),
            remediation: Some("upgrade the calling component".into()),
            details: None,
        });
    }
    if req.id == 0 {
        return Err(IpcError {
            code: ErrorCode::ParseFailed,
            message: "request id must be non-zero".into(),
            remediation: None,
            details: None,
        });
    }
    Ok(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_round_trip() {
        let req = Request::new(7, Method::ScanDevices, json!({}));
        let line = serde_json::to_string(&req).unwrap();
        let back = parse_request(&line).unwrap();
        assert_eq!(back.id, 7);
        assert_eq!(back.method, Method::ScanDevices);
        assert_eq!(back.timeout(), DEFAULT_TIMEOUT_MS);
    }

    #[test]
    fn rejects_future_protocol_version() {
        let mut req = Request::new(1, Method::Hello, json!({}));
        req.protocol_version = 99;
        let line = serde_json::to_string(&req).unwrap();
        let err = parse_request(&line).unwrap_err();
        assert_eq!(err.code, ErrorCode::UnsupportedSchema);
    }

    #[test]
    fn rejects_id_zero() {
        let mut req = Request::new(0, Method::Hello, json!({}));
        req.id = 0;
        let line = serde_json::to_string(&req).unwrap();
        let err = parse_request(&line).unwrap_err();
        assert_eq!(err.code, ErrorCode::ParseFailed);
    }

    #[test]
    fn response_line_ends_with_newline() {
        let r = Response::ok(3, json!({"ok": true}), 12);
        let line = r.to_line().unwrap();
        assert!(line.ends_with('\n'));
        assert!(line.contains("\"status\":\"ok\""));
    }

    #[test]
    fn typed_errors_are_structured() {
        let e = crate::error::CamarelolError::new(ErrorCode::SafetyGateClosed, "nope")
            .with_remediation("arm the gate");
        let e: IpcError = e.into();
        assert_eq!(e.code, ErrorCode::SafetyGateClosed);
        assert!(e.remediation.is_some());
    }
}
