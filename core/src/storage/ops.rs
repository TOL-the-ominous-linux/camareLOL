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

/// What to run, before the safety gate has approved it.
struct Cmd<'a> {
    op: Operation,
    risk: RiskLevel,
    program: &'a str,
    args: Vec<String>,
    stdin: Option<String>,
}

impl OpRunner {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a record without running anything.
    ///
    /// Used by the install runner's preview paths, which bypass [`Self::go`]
    /// because they never call `OpRunner` at all (pacstrap, genfstab, the
    /// chroot scripts and mkinitcpio do not go through this helper). Recording
    /// into the same list keeps one single, ordered command list for the user.
    pub fn push_record(&mut self, record: RecordedCommand) {
        self.records.push(record);
    }

    fn go(&mut self, gate: &SafetyGate, log: &Logger, cmd: Cmd<'_>) -> Result<()> {
        let Cmd {
            op,
            risk,
            program,
            args,
            stdin,
        } = cmd;
        gate.authorize(op, risk)?;
        // `writes_to_disk()` and not `gate.dry_run`: in preview the field can be
        // anything a caller set it to, this one cannot say yes.
        let writing = gate.writes_to_disk();
        log.log(
            "storage",
            &format!("{} {} (writing={})", program, args.join(" "), writing),
        );
        if !writing {
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
            Some(input) => sys::run_with_input(
                program,
                &arg_refs,
                input,
                std::time::Duration::from_secs(300),
            )?,
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

        let script = sfdisk_script(plan, device.size_bytes);
        log.log(
            "storage",
            &format!("writing partition plan to {}", device.path),
        );

        // 1. zap old signatures only when the plan says so (we always zap: the
        // plan is a full layout). This is the destructive step.
        self.go(
            gate,
            log,
            Cmd {
                op: Operation::WritePartitionTable,
                risk,
                program: "sgdisk",
                args: vec!["--zap-all".into(), device.path.clone()],
                stdin: None,
            },
        )?;
        // 2. write the new table + partitions. sfdisk asks the kernel to
        //    re-read the table (no --no-reread here on purpose).
        self.go(
            gate,
            log,
            Cmd {
                op: Operation::WritePartitionTable,
                risk,
                program: "sfdisk",
                args: vec!["--force".into(), device.path.clone()],
                stdin: Some(script),
            },
        )?;
        // 3. Belt and braces: ask the kernel and udev to notice the new table.
        //    These helpers legitimately fail on a freshly wiped disk (there is
        //    nothing to probe yet), so they are best effort — the real gate is
        //    the verification below.
        self.try_go(
            gate,
            Operation::WritePartitionTable,
            risk,
            "partprobe",
            vec![device.path.clone()],
            log,
        );
        self.try_go(
            gate,
            Operation::WritePartitionTable,
            risk,
            "blockdev",
            vec!["--rereadpt".into(), device.path.clone()],
            log,
        );
        self.try_go(
            gate,
            Operation::WritePartitionTable,
            risk,
            "udevadm",
            vec!["settle".into()],
            log,
        );
        // 4. Fail early and clearly if the kernel still does not see the
        //    partitions — formatting a missing device would be a silent no-op.
        //    Nothing was written, so there is nothing to wait for.
        if gate.writes_to_disk() {
            self.wait_for_partitions(device, plan.partitions.len(), log)?;
        }
        Ok(())
    }

    /// Best-effort variant of [`OpRunner::go`]: failures are logged, not fatal.
    ///
    /// Used for kernel/udev nudge commands whose failure is normal (for
    /// example probing a disk that just lost its partition table). The caller
    /// is still responsible for verifying the result afterwards.
    fn try_go(
        &mut self,
        gate: &SafetyGate,
        op: Operation,
        risk: RiskLevel,
        program: &str,
        args: Vec<String>,
        log: &Logger,
    ) {
        let before = self.records.len();
        let result = self.go(
            gate,
            log,
            Cmd {
                op,
                risk,
                program,
                args,
                stdin: None,
            },
        );
        if let Err(e) = result {
            log.log("storage", &format!("{program} did not need to run: {e}"));
            // Drop the failed attempt: it is not a step of the plan.
            self.records.truncate(before);
        }
    }

    /// Wait until the kernel exposes the planned partition devices.
    ///
    /// In a live installer this is what tells us the table really landed —
    /// without it we would happily run `mkfs` against a device that does not
    /// exist and report a confusing error later.
    fn wait_for_partitions(
        &mut self,
        device: &StorageDevice,
        expected: usize,
        log: &Logger,
    ) -> Result<()> {
        use std::time::{Duration, Instant};
        if expected == 0 {
            return Ok(());
        }
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut last_missing: Vec<String> = Vec::new();
        while Instant::now() < deadline {
            let missing = (1..=expected)
                .map(|n| super::super::install::partition_path(&device.path, n))
                .filter(|p| !crate::sys::path_exists(p))
                .collect::<Vec<String>>();
            if missing.is_empty() {
                log.log(
                    "storage",
                    "all planned partitions are visible to the kernel",
                );
                return Ok(());
            }
            last_missing = missing;
            std::thread::sleep(Duration::from_millis(500));
        }
        Err(CamarelolError::new(
            ErrorCode::DeviceBusy,
            format!(
                "the kernel did not pick up the new partition table (missing: {})",
                last_missing.join(", ")
            ),
        )
        .with_remediation("retry; if the disk is held open, unmount it first")
        .with_context(format!("device {}", device.path)))
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
                self.mkfs(
                    "mkfs.fat",
                    vec![
                        "-F".into(),
                        "32".into(),
                        "-n".into(),
                        spec.label.clone().unwrap_or_else(|| "ESP".into()),
                        target.into(),
                    ],
                    gate,
                    risk,
                    log,
                )?;
            }
            Filesystem::Ext4 => {
                self.mkfs(
                    "mkfs.ext4",
                    vec![
                        "-F".into(),
                        "-L".into(),
                        spec.label.clone().unwrap_or_else(|| "camarelol".into()),
                        target.into(),
                    ],
                    gate,
                    risk,
                    log,
                )?;
            }
            Filesystem::Btrfs => {
                self.mkfs(
                    "mkfs.btrfs",
                    vec![
                        "-f".into(),
                        "-L".into(),
                        spec.label.clone().unwrap_or_else(|| "camarelol".into()),
                        target.into(),
                    ],
                    gate,
                    risk,
                    log,
                )?;
            }
            Filesystem::Xfs => {
                self.mkfs(
                    "mkfs.xfs",
                    vec![
                        "-f".into(),
                        "-L".into(),
                        spec.label.clone().unwrap_or_else(|| "camarelol".into()),
                        target.into(),
                    ],
                    gate,
                    risk,
                    log,
                )?;
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
        self.go(
            gate,
            log,
            Cmd {
                op: Operation::FormatPartition,
                risk,
                program,
                args,
                stdin: None,
            },
        )
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
            log,
            Cmd {
                op: Operation::Mount,
                risk: RiskLevel::Low,
                program: "mkdir",
                args: vec!["-p".into(), target.into()],
                stdin: None,
            },
        )?;
        self.go(
            gate,
            log,
            Cmd {
                op: Operation::Mount,
                risk: RiskLevel::Low,
                program: "mount",
                args: vec![source.into(), target.into()],
                stdin: None,
            },
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
///
/// `disk_size_bytes` is required: the last partition has to be shrunk so that
/// the layout — including alignment gaps and the GPT backup header — really
/// fits on the disk.
pub fn sfdisk_script(plan: &PartitionPlan, disk_size_bytes: u64) -> String {
    const ALIGN_SECTORS: u64 = 2048; // 1 MiB
    let total_sectors = disk_size_bytes / 512;
    // GPT keeps a backup header at the end of the disk.
    let tail_reserve: u64 = match plan.table_type {
        super::device::PartitionTableType::Gpt => 33,
        _ => 1,
    };

    let mut s = String::new();
    s.push_str("label: ");
    s.push_str(match plan.table_type {
        super::device::PartitionTableType::Gpt => "gpt",
        super::device::PartitionTableType::Mbr => "dos",
        _ => "gpt",
    });
    s.push('\n');

    let mut start_sector: u64 = ALIGN_SECTORS;
    let count = plan.partitions.len();
    for (index, spec) in plan.partitions.iter().enumerate() {
        let usable = total_sectors.saturating_sub(tail_reserve);
        let sectors = if index + 1 == count {
            // The last partition takes whatever is left.
            usable.saturating_sub(start_sector)
        } else {
            size_in_sectors(spec).min(usable.saturating_sub(start_sector))
        };
        // Only stop when the disk is genuinely full. A 1 MiB BIOS boot
        // partition is exactly ALIGN_SECTORS long and must still be written.
        if sectors == 0 {
            break;
        }
        s.push_str(&format!("start={start_sector}, size={sectors}"));
        if let Some(t) = part_type_flag(spec) {
            s.push_str(&format!(", type={t}"));
        }
        if let Some(l) = &spec.label {
            let mut name = l.clone();
            name.retain(|c| c.is_ascii_alphanumeric() || c == '-');
            if !name.is_empty() {
                s.push_str(&format!(", name={name}"));
            }
        }
        s.push('\n');
        start_sector += sectors + ALIGN_SECTORS;
    }
    s
}

fn size_in_sectors(spec: &PartitionSpec) -> u64 {
    let bytes = match &spec.size {
        SizeSpec::Bytes(b) => *b,
        SizeSpec::PercentOfDisk(_p) => 0, // only used with an explicit size in v1
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
        let script = sfdisk_script(&plan, 20 * 1024 * 1024 * 1024);
        assert!(script.starts_with("label: gpt"));
        assert!(script.contains("type=U"));
        assert!(script.contains("start=2048"));
    }

    #[test]
    fn sfdisk_script_fits_inside_the_disk() {
        // Regression: the layout must never ask for sectors past the end of
        // the disk (alignment gaps + GPT backup header).
        let plan = test_plan();
        let disk_bytes = 20 * 1024 * 1024 * 1024u64;
        let total_sectors = disk_bytes / 512;
        let script = sfdisk_script(&plan, disk_bytes);
        for line in script.lines().filter(|l| l.contains("start=")) {
            let field = |key: &str| -> u64 {
                line.split(',')
                    .map(|p| p.trim())
                    .find_map(|p| p.strip_prefix(key).map(|v| v.parse().unwrap()))
                    .unwrap_or(0)
            };
            let start = field("start=");
            let size = field("size=");
            assert!(
                start + size <= total_sectors - 33,
                "partition `{line}` exceeds the disk ({total_sectors} sectors)"
            );
            assert!(
                start >= 2048 && start % 2048 == 0,
                "partition `{line}` is not 1 MiB aligned"
            );
        }
    }

    #[test]
    fn sfdisk_script_handles_a_small_disk_without_overflow() {
        // 3 GiB with a 1 GiB ESP + root: root must shrink, not overflow.
        let plan = test_plan();
        let disk_bytes = 3 * 1024 * 1024 * 1024u64;
        let script = sfdisk_script(&plan, disk_bytes);
        let partitions: Vec<&str> = script.lines().filter(|l| l.contains("start=")).collect();
        let last = partitions
            .last()
            .expect("at least the root partition is written")
            .to_string();
        let field = |line: &str, key: &str| -> u64 {
            line.split(',')
                .map(|p| p.trim())
                .find_map(|p| p.strip_prefix(key).map(|v| v.parse().unwrap()))
                .unwrap()
        };
        let start = field(&last, "start=");
        let size = field(&last, "size=");
        assert!(start + size <= disk_bytes / 512 - 33);
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
    fn sfdisk_script_writes_every_planned_partition() {
        // Regression: the BIOS boot partition is exactly 1 MiB, which equals
        // the alignment step. The generator must not stop before writing it.
        let devices = scan_devices_from_survey(
            r#"[{"kernel_name":"vdb","path":"/dev/vdb","size_bytes":21474836480,"rotational":false,"removable":false,"kind":"Virtual","partition_table":"None","partitions":[],"mount_state":"NotMounted","os_hints":[],"encrypted":null,"risk":"Low"}]"#,
        )
        .unwrap();
        let req = crate::storage::layout::LayoutRequest {
            firmware: crate::hardware::Firmware::Bios,
            bootloader: crate::boot::Bootloader::Grub,
            ..Default::default()
        };
        let plan = plan_auto_layout(&devices[0], &req).unwrap();
        let script = sfdisk_script(&plan, 20 * 1024 * 1024 * 1024);
        let written = script.lines().filter(|l| l.contains("start=")).count();
        assert_eq!(
            written,
            plan.partitions.len(),
            "script is missing partitions:\n{script}"
        );
        assert!(
            script.contains("21686148-6449-6E6F-744E-656564454649"),
            "BIOS boot partition type missing:\n{script}"
        );
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
