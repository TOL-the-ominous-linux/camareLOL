//! camareLOL core.
//!
//! This crate is the **authoritative** system core. The TUI, the Python tools and
//! the Java catalog all send requests to it; every system-changing request is
//! validated here and passes through [`crate::safety::SafetyGate`].
//!
//! ```text
//!   tui (ratatui)  ┐
//!   python tools   ├──► camarelol-core ──► sfdisk / mkfs / pacstrap / arch-chroot
//!   java catalog   ┘        (validated, gated, logged)
//! ```

pub mod accounts;
pub mod boot;
pub mod config;
pub mod desktop;
pub mod error;
pub mod hardware;
pub mod install;
pub mod locale;
pub mod logging;
pub mod network;
pub mod packages;
pub mod protocol;
pub mod recovery;
pub mod safety;
pub mod services;
pub mod storage;
pub mod sys;

use crate::error::{CamarelolError, ErrorCode, Result};
use crate::install::execute::SystemRunner;
use crate::protocol::{Method, Request, Response, PROTOCOL_VERSION};
use crate::safety::SafetyGate;
use serde_json::{json, Value};
use std::time::Instant;

/// The version of the whole core.
pub const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Everything the TUI/CLI needs in one place.
pub struct Core {
    pub config: config::InstallationConfig,
    pub gate: SafetyGate,
    pub devices: Vec<storage::StorageDevice>,
    log: logging::Logger,
    boot: boot::Bootloader,
    firmware: hardware::Firmware,
}

impl Core {
    /// Create a core instance. `detect` performs a fresh hardware/disk survey.
    pub fn new(detected_firmware: hardware::Firmware) -> Self {
        let log = logging::Logger::new(std::path::Path::new("/var/log/camarelol"));
        log.log(
            "core",
            &format!(
                "camareLOL {} starting (protocol v{CORE_VERSION})",
                CORE_VERSION
            ),
        );
        let devices = storage::scan::scan_devices().unwrap_or_default();
        let cfg = config::InstallationConfig::default();
        Core {
            config: cfg,
            gate: SafetyGate::default(),
            devices,
            log,
            boot: boot::Bootloader::Grub,
            firmware: detected_firmware,
        }
    }

    /// Create a core with a mock device survey (used by tests and `--mock`).
    /// Create a core with a known set of devices.
    ///
    /// The firmware is *detected*, never assumed: hard-coding UEFI here made
    /// `camarelol --self-test` fail on BIOS machines, because the plan was
    /// built for UEFI while the install resolved the real (BIOS) mode.
    pub fn with_devices(devices: Vec<storage::StorageDevice>) -> Self {
        Self::with_devices_and_firmware(devices, hardware::detect_firmware())
    }

    /// Same as [`Core::with_devices`] but with an explicit firmware mode.
    ///
    /// Tests use this so they behave identically on a UEFI workstation, on a
    /// BIOS machine and inside a container (where detection legitimately
    /// returns `Unknown` and the planner correctly refuses to proceed).
    pub fn with_devices_and_firmware(
        devices: Vec<storage::StorageDevice>,
        firmware: hardware::Firmware,
    ) -> Self {
        Core {
            config: config::InstallationConfig::default(),
            gate: SafetyGate::default(),
            devices,
            log: logging::Logger::default(),
            boot: boot::Bootloader::Grub,
            firmware,
        }
    }

    pub fn firmware(&self) -> hardware::Firmware {
        self.firmware
    }

    pub fn set_firmware(&mut self, f: hardware::Firmware) {
        self.firmware = f;
    }

    pub fn bootloader(&self) -> boot::Bootloader {
        self.boot
    }

    pub fn set_bootloader(&mut self, b: boot::Bootloader) {
        self.boot = b;
    }

    pub fn log(&self) -> &logging::Logger {
        &self.log
    }

    pub fn device(&self, path: &str) -> Option<&storage::StorageDevice> {
        self.devices.iter().find(|d| d.path == path)
    }

    /// Build a partition plan for the given disk path.
    pub fn plan_for(&self, disk_path: &str) -> Result<storage::PartitionPlan> {
        let device = self.device(disk_path).ok_or_else(|| {
            CamarelolError::new(
                ErrorCode::NoTargetDisk,
                format!("no such device: {disk_path}"),
            )
        })?;
        let req = self.layout_request();
        let plan = storage::layout::plan_auto_layout(device, &req)?;
        storage::layout::validate_plan(&plan, &req)?;
        Ok(plan)
    }

    fn layout_request(&self) -> storage::layout::LayoutRequest {
        storage::layout::LayoutRequest {
            firmware: self.firmware,
            bootloader: self.boot,
            root_filesystem: self
                .config
                .filesystem_type()
                .unwrap_or(storage::Filesystem::Ext4),
            separate_home: self.config.separate_home,
            separate_boot: self.config.separate_boot,
            swap_bytes: self.config.swap_bytes,
        }
    }

    /// Validate the current plan against boot rules.
    pub fn validate_plan(&self, plan: &storage::PartitionPlan) -> Result<()> {
        let req = self.layout_request();
        storage::layout::validate_plan(plan, &req)?;
        boot::validate_boot_config(plan, self.firmware, self.boot)?;
        Ok(())
    }

    /// The single entry point for every request — TUI, Python or Java.
    ///
    /// A destructive method can only succeed when the gate is armed **and** the
    /// user has confirmed it; the confirmation is held by the caller (the UI) and
    /// given to [`SafetyGate`] before this call.
    pub fn dispatch(&mut self, req: Request) -> Response {
        let started = Instant::now();
        let id = req.id;
        let result: std::result::Result<Value, CamarelolError> = match req.method {
            Method::Hello => Ok(json!({
                "core": "camareLOL",
                "version": CORE_VERSION,
                "protocol_version": PROTOCOL_VERSION,
                "gate": self.gate.mode().as_str(),
            })),
            Method::ScanDevices => Ok(json!({ "devices": self.devices })),
            Method::CreatePlan => {
                let path = req
                    .params
                    .get("disk")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                self.plan_for(&path)
                    .map(|p| serde_json::to_value(p).unwrap())
            }
            Method::ValidatePlan => {
                let path = req
                    .params
                    .get("disk")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                self.plan_for(&path).and_then(|p| {
                    self.validate_plan(&p)?;
                    Ok(json!({"valid": true}))
                })
            }
            Method::ApplyPlan => {
                // Requires an armed gate and confirmation; validation happens
                // again right here, in the core.
                let path = req
                    .params
                    .get("disk")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                self.plan_for(&path).and_then(|p| {
                    self.validate_plan(&p)?;
                    let device = self.device(&path).cloned().ok_or_else(|| {
                        CamarelolError::new(ErrorCode::DeviceGone, format!("device {path} is gone"))
                    })?;
                    let mut runner = install::execute::RealSystemRunner::new(self.gate.clone());
                    let log = logging::Logger::default();
                    runner
                        .apply_partition_table(&device, &p)
                        .map(|_| json!({"applied": true, "records": runner.records().len() }))
                        .map_err(|e| {
                            log.log("ipc", &format!("ApplyPlan refused: {e}"));
                            e
                        })
                })
            }
            Method::FormatPartitions => Err(CamarelolError::new(
                ErrorCode::NotSupported,
                "formatting is driven by the install orchestrator in v0.1",
            )),
            Method::Diagnostics => {
                let report = recovery::collect(&[]);
                Ok(json!({ "report": report }))
            }
            Method::GetConfig => Ok(serde_json::to_value(&self.config).unwrap_or(json!({}))),
            Method::SetConfig => {
                match serde_json::from_value::<config::InstallationConfig>(req.params.clone()) {
                    Ok(mut cfg) => match cfg.validate() {
                        Ok(()) => {
                            let passwords = std::mem::take(&mut cfg.passwords);
                            self.config = cfg;
                            self.config.passwords = passwords;
                            Ok(json!({"saved": true}))
                        }
                        Err(e) => Err(e),
                    },
                    Err(e) => Err(CamarelolError::new(
                        ErrorCode::InvalidConfig,
                        format!("invalid configuration payload: {e}"),
                    )),
                }
            }
            Method::RecommendPackages => {
                // Informational only: the recommendation engine lives in Python.
                Ok(json!({"status": "delegated", "component": "python"}))
            }
        };
        let took = started.elapsed().as_millis();
        match result {
            Ok(v) => Response::ok(id, v, took),
            Err(e) => Response::err(id, e.into(), took),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::InstallationConfig;
    use crate::protocol::{parse_request, Method, Request, ResponseStatus};

    fn core() -> Core {
        // Pin the firmware: inside a container detection returns Unknown, and
        // the planner (correctly) refuses to build a plan in that case.
        Core::with_devices_and_firmware(
            storage::scan::scan_devices_from_survey(
                r#"[{"kernel_name":"vdb","path":"/dev/vdb","size_bytes":21474836480,"rotational":false,"removable":false,"kind":"Virtual","partition_table":"None","partitions":[],"mount_state":"NotMounted","os_hints":[],"encrypted":null,"risk":"Low"}]"#,
            )
            .unwrap(),
            hardware::Firmware::Uefi,
        )
    }

    #[test]
    fn planning_works_for_every_known_firmware_mode() {
        // A container has neither efivars nor ACPI tables, so detection yields
        // Unknown there. The planner must stay testable regardless of host.
        let devices = storage::scan::scan_devices_from_survey(
            r#"[{"kernel_name":"vdb","path":"/dev/vdb","size_bytes":21474836480,"rotational":false,"removable":false,"kind":"Virtual","partition_table":"None","partitions":[],"mount_state":"NotMounted","os_hints":[],"encrypted":null,"risk":"Low"}]"#,
        )
        .unwrap();
        for mode in [hardware::Firmware::Uefi, hardware::Firmware::Bios] {
            let core = Core::with_devices_and_firmware(devices.clone(), mode);
            let plan = core
                .plan_for("/dev/vdb")
                .unwrap_or_else(|e| panic!("plan failed for {}: {e}", mode.as_str()));
            core.validate_plan(&plan)
                .unwrap_or_else(|e| panic!("validate failed for {}: {e}", mode.as_str()));
        }
    }

    #[test]
    fn planning_refuses_when_the_firmware_is_unknown() {
        // Correct behaviour: rather than guess, refuse and explain.
        let devices = storage::scan::scan_devices_from_survey(
            r#"[{"kernel_name":"vdb","path":"/dev/vdb","size_bytes":21474836480,"rotational":false,"removable":false,"kind":"Virtual","partition_table":"None","partitions":[],"mount_state":"NotMounted","os_hints":[],"encrypted":null,"risk":"Low"}]"#,
        )
        .unwrap();
        let core = Core::with_devices_and_firmware(devices, hardware::Firmware::Unknown);
        let err = core.plan_for("/dev/vdb").unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidBootConfig);
    }

    #[test]
    fn hello_returns_protocol_version() {
        let mut c = core();
        let r = c.dispatch(Request::new(1, Method::Hello, json!({})));
        assert_eq!(r.status, ResponseStatus::Ok);
        assert_eq!(r.result.unwrap()["protocol_version"], 1);
    }

    #[test]
    fn scan_devices_lists_the_survey() {
        let mut c = core();
        let r = c.dispatch(Request::new(2, Method::ScanDevices, json!({})));
        let devices = r.result.unwrap()["devices"].as_array().unwrap().len();
        assert_eq!(devices, 1);
    }

    #[test]
    fn apply_plan_is_refused_by_the_locked_gate() {
        let mut c = core();
        let r = c.dispatch(Request::new(
            3,
            Method::ApplyPlan,
            json!({"disk": "/dev/vdb"}),
        ));
        assert_eq!(r.status, ResponseStatus::Error);
        assert_eq!(r.error.unwrap().code, ErrorCode::SafetyGateClosed);
    }

    #[test]
    fn set_config_validates_before_accepting() {
        let mut c = core();
        let mut payload = serde_json::to_value(InstallationConfig::default()).unwrap();
        payload["schema_version"] = json!(99);
        let r = c.dispatch(Request::new(4, Method::SetConfig, payload));
        assert_eq!(r.status, ResponseStatus::Error);
        assert_eq!(r.error.unwrap().code, ErrorCode::UnsupportedSchema);
    }

    #[test]
    fn set_config_accepts_a_valid_payload() {
        let mut c = core();
        let payload = serde_json::to_value(InstallationConfig::default()).unwrap();
        let r = c.dispatch(Request::new(5, Method::SetConfig, payload));
        assert_eq!(r.status, ResponseStatus::Ok);
    }

    #[test]
    fn parse_request_round_trip() {
        let req = Request::new(9, Method::Diagnostics, json!({}));
        let line = serde_json::to_string(&req).unwrap();
        let parsed = parse_request(&line).unwrap();
        assert_eq!(parsed.method, Method::Diagnostics);
    }

    #[test]
    fn plan_and_validate_for_empty_disk() {
        let c = core();
        let plan = c.plan_for("/dev/vdb").unwrap();
        c.validate_plan(&plan).unwrap();
    }
}
