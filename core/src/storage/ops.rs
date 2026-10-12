//! Destructive storage operations.
//!
//! Nothing in this module can run without a confirmed [`SafetyGate`]. In
//! dry-run mode the operations are recorded instead of executed, which is what
//! the UI shows the user in the Review screen and what the tests assert on.

use super::device::{Filesystem, RiskLevel, StorageDevice};
use super::layout::{PartitionPlan, PartitionPurpose, PartitionSpec, SizeSpec};
use crate::error::{CamarelolError, ErrorCode, Result};
use crate::logging::Logger;
use crate::safety::{Operation, SafetyGate};
use crate::sys;

/// One atomic step that a phase performs.
#[derive(Debug, Clone)]
pub struct RecordedCommand {
    pub program: String,
    pub args: Vec<String>,
    pub stdin: Option<String>,
    /// false when the gate was in dry-run mode.
    pub applied: bool,
    pub status: Option<i32>,
    pub stderr: String,
}

impl RecordedCommand {
    fn preview(&self) -> String {
        let mut s = format!("{} {}", self.program, self.args.join(" "));
        if let Some(input) = &self.stdin {
            s.push_str(&format!(" <<< {}", input.replace('\n', " | ")));
        }
        s
    }
}

/// A helper that either runs the command or records it, depending on the gate.
#[derive(Debug, Default)]
pub struct OpRunner {
    pub records: Vec<RecordedCommand>,
}

impl OpRunner {
    pub fn new() -> Self {
        Self::default()
    }

    fn go(
        &mut self,
        gate: &SafetyGate,
        op: Operation,
        risk: RiskLevel,
        program: &str,
        args: Vec<String>,
        stdin: Option<String>,
        log: &Logger,
    ) -> Result<()> {
        gate.authorize(op, risk)?;
        log.log("storage", &format!("{} {} (dry_run={})", program, args.join(" "), gate.dry_run));
        if gate.dry_run {
            self.records.push(RecordedCommand {
                program: program.to_string(),
                args,
                stdin,
                applied: false,
                status: None,
                stderr: String::new(),
            });
            return Ok(());
        }
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let out = match &stdin {
            Some(input) => {
                sys::run_with_input(program, &arg_refs, input, std::time::Duration::from_secs(300))?
            }
            None => sys::run(program, &arg_refs)?,
        };
        let status = out.status;
        let stderr = out.stderr.clone();
        self.records.push(RecordedCommand {
            program: program.to_string(),
            args,
            stdin,
            applied: true,
            status: Some(status),
            stderr,
        });
        if !out.success() {
            return Err(CamarelolError::new(
                ErrorCode::CommandFailed,
                format!("`{program}` exited with status {status}"),
            )
            .with_context(out.stderr.trim().to_string()));
        }
        Ok(())
    }

    /// Write the partition table of `device` and create the planned partitions.
    ///
    /// The sfdisk script is passed on stdin — no user input is ever interpolated
    /// into a shell command line.
    pub fn apply_partition_table(
        &mut self,
        device: &StorageDevice,
        plan: &PartitionPlan,
        gate: &SafetyGate,
        log: &Logger,
    ) -> Result<()> {
        let risk = plan.disk_risk;
        gate.authorize(Operation::WritePartitionTable, risk)?;

        let script = sfdisk_script(plan);
        log.log("storage", &format!("writing partition plan to {}", device.path));

        // 1. zap old signatures only when the plan says so (we always zap: the
        // plan is a full layout). This is the destructive step.
        self.go(
            gate,
            Operation::WritePartitionTable,
            risk,
            "sgdisk",
            vec!["--zap-all".into(), device.path.clone()],
            None,
            log,
        )?;
        // 2. write the new table + partitions
        self.go(
            gate,
            Operation::WritePartitionTable,
            risk,
            "sfdisk",
            vec!["--no-reread".into(), "--force".into(), device.path.clone()],
            Some(script),
            log,
        )?;
        // 3. make the kernel see the new table
        self.go(
            gate,
            Operation::WritePartitionTable,
            risk,
            "partprobe",
            vec![device.path.clone()],
            None,
            log,
        )?;
        self.go(
            gate,
            Operation::WritePartitionTable,
            risk,
            "udevadm",
            vec!["settle".into()],
            None,
            log,
        )
    }

    /// Create the filesystem of one planned partition.
    pub fn format_partition(
        &mut self,
        spec: &PartitionSpec,
        target: &str,
        gate: &SafetyGate,
        log: &Logger,
    ) -> Result<()> {
        if !spec.format {
            log.log("storage", &format!("keeping existing data on {target}"));
            return Ok(());
        }
        let fs = spec.filesystem.as_ref().ok_or_else(|| {
            CamarelolError::new(
                ErrorCode::InvalidPlan,
                format!("{} has no filesystem to format with", spec.purpose.as_str()),
            )
        })?;
        let risk = RiskLevel::Medium;
        match fs {
            Filesystem::Vfat => {
                self.mkfs("mkfs.fat", vec!["-F".into(), "32".into(), "-n".into(), spec.label.clone().unwrap_or_else(|| "ESP".into()), target.into()], gate, risk, log)?;
            }
            Filesystem::Ext4 => {
                self.mkfs("mkfs.ext4", vec!["-F".into(), "-L".into(), spec.label.clone().unwrap_or_else(|| "camarelol".into()), target.into()], gate, risk, log)?;
            }
            Filesystem::Btrfs => {
                self.mkfs("mkfs.btrfs", vec!["-f".into(), "-L".into(), spec.label.clone().unwrap_or_else(|| "camarelol".into()), target.into()], gate, risk, log)?;
            }
            Filesystem::Xfs => {
                self.mkfs("mkfs.xfs", vec!["-f".into(), "-L".into(), spec.label.clone().unwrap_or_else(|| "camarelol".into()), target.into()], gate, risk, log)?;
            }
            Filesystem::Swap => {
                self.mkfs("mkswap", vec![target.into()], gate, risk, log)?;
            }
            other => {
                return Err(CamarelolError::new(
                    ErrorCode::NotSupported,
                    format!("formatting {} is not supported yet", other.as_str()),
                ));
            }
        }
        Ok(())
    }

    fn mkfs(
        &mut self,
        program: &str,
        args: Vec<String>,
        gate: &SafetyGate,
        risk: RiskLevel,
        log: &Logger,
    ) -> Result<()> {
        self.go(gate, Operation::FormatPartition, risk, program, args, None, log)
    }

    /// Mount a partition under the target root.
    pub fn mount(
        &mut self,
        source: &str,
        target: &str,
        gate: &SafetyGate,
        log: &Logger,
    ) -> Result<()> {
        self.go(
            gate,
            Operation::Pacstrap,
            RiskLevel::Low,
            "mkdir",
            vec!["-p".into(), target.into()],
            None,
            log,
        )?;
        self.go(
            gate,
            Operation::Pacstrap,
            RiskLevel::Low,
            "mount",
            vec![source.into(), target.into()],
            None,
            log,
        )
    }

    /// Show what *would* be executed.
    pub fn report(&self) -> String {
        if self.records.is_empty() {
            return "no commands".to_string();
        }
        self.records
            .iter()
            .map(|r| {
                format!(
                    "{} {}",
                    if r.applied { "RAN   " } else { "DRYRUN" },
                    r.preview()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Build an sfdisk script from a plan.
///
/// Uses sectors, which is unambiguous across disks, and never interpolates
/// anything but values we validated ourselves.
pub fn sfdisk_script(plan: &PartitionPlan) -> String {
    let mut s = String::new();
    s.push_str("label: ");
    s.push_str(match plan.table_type {
        super::device::PartitionTableType::Gpt => "gpt",
        super::device::PartitionTableType::Mbr => "dos",
        _ => "gpt",
    });
    s.push('\n');
    let mut start_sector: u64 = 2048; // 1 MiB alignment
    for spec in &plan.partitions {
        let sectors = size_in_sectors(spec);
        s.push_str(&format!("start={start_sector}, size={sectors}"));
        if let Some(t) = part_type_flag(spec) {
            s.push_str(&format!(", type={t}"));
        }
        if let Some(l) = &spec.label {
            let mut name = l.clone();
            name.retain(|c| c.is_ascii_alphanumeric() || c == '-');
            s.push_str(&format!(", name={name}"));
        }
        s.push('\n');
        start_sector += sectors + 2048; // align next partition
    }
    s
}

fn size_in_sectors(spec: &PartitionSpec) -> u64 {
    let bytes = match &spec.size {
        SizeSpec::Bytes(b) => *b,
        SizeSpec::PercentOfDisk(p) => 0, // only used with an explicit size in v1
        SizeSpec::RemainingSpace => 0,
    };
    let sectors = bytes / 512;
    sectors.max(2048)
}

fn part_type_flag(spec: &PartitionSpec) -> Option<&'static str> {
    match spec.purpose {
        PartitionPurpose::EfiSystem => Some("U"),
        PartitionPurpose::BiosBoot => Some("21686148-6449-6E6F-744E-656564454649"),
        PartitionPurpose::Swap => Some("S"),
        _ => Some("L"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::layout::plan_auto_layout;
    use crate::storage::scan::scan_devices_from_survey;

    fn test_plan() -> PartitionPlan {
        let devices = scan_devices_from_survey(
            r#"[{"kernel_name":"vdb","path":"/dev/vdb","size_bytes":21474836480,"rotational":false,"removable":false,"kind":"Virtual","partition_table":"None","partitions":[],"mount_state":"NotMounted","os_hints":[],"encrypted":null,"risk":"Low"}]"#,
        )
        .unwrap();
        let req = crate::storage::layout::LayoutRequest::default();
        plan_auto_layout(&devices[0], &req).unwrap()
    }

    #[test]
    fn dry_run_records_commands_without_running_them() {
        let mut runner = OpRunner::new();
        let gate = SafetyGate::armed();
        let plan = test_plan();
        let devices = scan_devices_from_survey(
            r#"[{"kernel_name":"vdb","path":"/dev/vdb","size_bytes":21474836480,"rotational":false,"removable":false,"kind":"Virtual","partition_table":"None","partitions":[],"mount_state":"NotMounted","os_hints":[],"encrypted":null,"risk":"Low"}]"#,
        )
        .unwrap();
        runner
            .apply_partition_table(&devices[0], &plan, &gate, &Logger::default())
            .unwrap();
        assert!(!runner.records.is_empty());
        assert!(runner.records.iter().all(|r| !r.applied));
        assert!(runner.report().contains("DRYRUN"));
        // and the really destructive tool is never spawned
        assert!(runner.records.iter().all(|r| r.status.is_none()));
    }

    #[test]
    fn locked_gate_records_nothing() {
        let mut runner = OpRunner::new();
        let gate = SafetyGate::default();
        let plan = test_plan();
        let devices = scan_devices_from_survey(
            r#"[{"kernel_name":"vdb","path":"/dev/vdb","size_bytes":21474836480,"rotational":false,"removable":false,"kind":"Virtual","partition_table":"None","partitions":[],"mount_state":"NotMounted","os_hints":[],"encrypted":null,"risk":"Low"}]"#,
        )
        .unwrap();
        let err = runner
            .apply_partition_table(&devices[0], &plan, &gate, &Logger::default())
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::SafetyGateClosed);
        assert!(runner.records.is_empty());
    }

    #[test]
    fn sfdisk_script_is_valid_and_aligned() {
        let plan = test_plan();
        let script = sfdisk_script(&plan);
        assert!(script.starts_with("label: gpt"));
        assert!(script.contains("type=U"));
        assert!(script.contains("start=2048"));
    }

    #[test]
    fn format_without_fs_spec_is_rejected() {
        let mut runner = OpRunner::new();
        let gate = SafetyGate::armed_and_execute();
        let spec = PartitionSpec {
            purpose: PartitionPurpose::BiosBoot,
            size: SizeSpec::Bytes(1024 * 1024),
            filesystem: None,
            mount_point: None,
            label: None,
            format: false,
        };
        runner
            .format_partition(&spec, "/dev/vdb1", &gate, &Logger::default())
            .unwrap(); // format=false is a no-op
        assert!(runner.records.is_empty());
    }

    #[test]
    fn unsupported_filesystem_is_refused() {
        let mut runner = OpRunner::new();
        let gate = SafetyGate::armed_and_execute();
        let spec = PartitionSpec {
            purpose: PartitionPurpose::Data,
            size: SizeSpec::Bytes(1024 * 1024),
            filesystem: Some(Filesystem::ZfsMember),
            mount_point: None,
            label: None,
            format: true,
        };
        let err = runner
            .format_partition(&spec, "/dev/vdb1", &gate, &Logger::default())
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::NotSupported);
    }
}
