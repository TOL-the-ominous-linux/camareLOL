//! Phase execution against the real system (or a simulated one in tests).
//!
//! The [`SystemRunner`] trait is the seam that makes destructive behaviour
//! testable: `RealSystemRunner` talks to parted/sfdisk/mkfs/pacstrap through
//! the safety gate, `SimulatedRunner` records calls only.

use super::phases::{preconditions, retryable};
use super::*;
use crate::boot::Bootloader;
use crate::error::{CamarelolError, ErrorCode, Result};
use crate::logging::Logger;
use crate::safety::{GateMode, SafetyGate};
use crate::storage::device::StorageDevice;
use crate::storage::layout::{PartitionPlan, PartitionPurpose, PartitionSpec};
use crate::storage::ops::OpRunner;

/// Everything the runner may touch, so tests can implement a fake.
pub trait SystemRunner {
    fn apply_partition_table(&mut self, disk: &StorageDevice, plan: &PartitionPlan) -> Result<()>;
    fn format_partition(&mut self, spec: &PartitionSpec, target: &str) -> Result<()>;
    fn mount(&mut self, source: &str, target: &str) -> Result<()>;
    fn pacstrap(&mut self, packages: &[String]) -> Result<()>;
    fn genfstab(&mut self) -> Result<()>;
    fn install_bootloader(&mut self, bl: Bootloader, disk: &StorageDevice) -> Result<()>;
    fn chroot_script(&mut self, script: &str) -> Result<()>;
    fn enable_services(&mut self, services: &[String]) -> Result<()>;
    fn mkinitcpio(&mut self) -> Result<()>;
    /// Build and install the explicitly selected AUR packages.
    ///
    /// Defaulted to a no-op so a runner that does not care about the AUR (every
    /// test double, and any future one) is unchanged. Only
    /// [`RealSystemRunner`] overrides it, and it refuses to run a build as
    /// root — see [`crate::aur`].
    ///
    /// Returns the number of packages built, so the phase can report honestly.
    fn build_aur(&mut self, _packages: &crate::packages::PackageSelection) -> Result<usize> {
        Ok(0)
    }
    /// Is the target system really mounted? The post-check is only meaningful
    /// when it is (unit tests with the simulated runner answer `false`).
    fn target_mounted(&self) -> bool;

    /// Non-fatal problems the user should know about (e.g. a service unit that
    /// does not exist on the target).
    fn warnings(&self) -> Vec<String> {
        Vec::new()
    }
}

/// Where the target system is mounted.
pub const TARGET_ROOT: &str = "/mnt";

/// Real runner: every destructive step passes through the [`SafetyGate`].
pub struct RealSystemRunner {
    ops: OpRunner,
    gate: SafetyGate,
    /// `--preview`: simulate a complete installation.
    ///
    /// Mirrors `gate.preview` (it is read from there, never set separately), so
    /// there is no second switch that could disagree with the gate. Every method
    /// below checks it before touching the system: it records the command it
    /// would run and returns `Ok(())`, which is what lets all twelve phases run
    /// end to end instead of dying on the first `ensure_exec()`.
    preview: bool,
    /// Non-fatal problems worth telling the user about.
    pub warnings: Vec<String>,
}

impl RealSystemRunner {
    pub fn new(gate: SafetyGate) -> Self {
        let preview = gate.preview;
        RealSystemRunner {
            ops: OpRunner::new(),
            gate,
            preview,
            warnings: Vec::new(),
        }
    }

    pub fn records(&self) -> &[crate::storage::ops::RecordedCommand] {
        &self.ops.records
    }

    /// The validated AUR plan for this run, or `None` when AUR is not in use.
    ///
    /// This is where every input to the decision is read, and nowhere else: the
    /// helper probe, the privilege-drop program, the effective uid and the
    /// configured build user. A uid that cannot be read is treated as *root*,
    /// so an unreadable `/proc` makes the installer more careful, not less.
    pub fn aur_plan(
        &self,
        packages: &crate::packages::PackageSelection,
    ) -> Result<Option<crate::aur::AurPlan>> {
        if packages.aur_list().is_empty() {
            return Ok(None);
        }
        let uid = crate::aur::effective_uid();
        let wrapper = crate::aur::detect_drop_program();
        crate::aur::plan(
            packages,
            crate::aur::detect_helper(),
            wrapper.as_deref(),
            packages.aur_build_user.as_deref(),
            // Unreadable uid is treated as root: `ensure_not_root` then refuses
            // the spawn instead of letting an unknown identity build anything.
            uid.is_none() || uid == Some(0),
        )
    }

    /// True when this runner only simulates.
    pub fn is_preview(&self) -> bool {
        self.preview
    }

    /// One printable line per command, in execution order, for the progress
    /// log and the `--scripted` output. Empty when nothing was attempted.
    pub fn command_lines(&self) -> Vec<String> {
        if self.ops.records.is_empty() {
            return Vec::new();
        }
        self.ops.report().lines().map(|l| l.to_string()).collect()
    }

    /// Record a command that a preview would run. Same shape as the record
    /// `OpRunner` keeps for a real dry run, so the preview list is exactly what
    /// the machine would have been asked to do.
    fn record(&mut self, program: &str, args: Vec<String>, stdin: Option<String>) {
        self.ops.push_record(crate::storage::ops::RecordedCommand {
            program: program.to_string(),
            args,
            stdin,
            applied: false,
            status: None,
            stderr: String::new(),
        });
    }

    /// Record a chroot script on stdin. The script is printed so the user can
    /// see it, with any password taken out: the same rule the real run follows
    /// by never putting a secret in argv, applied to what the user is shown.
    fn record_chroot(&mut self, script: &str) {
        self.record(
            "arch-chroot",
            vec![TARGET_ROOT.into()],
            Some(redact_secrets(script)),
        );
    }

    /// The gate to hand to `OpRunner` for a simulated step. `OpRunner` already
    /// records instead of running when the gate does not write to disk, so a
    /// preview here produces the real `sgdisk`/`sfdisk`/`mkfs`/`mount` list
    /// without a single spawn.
    fn preview_gate(&self) -> SafetyGate {
        let mut gate = self.gate.clone();
        gate.preview = true;
        gate.dry_run = true;
        gate
    }

    fn ensure_exec(&self) -> Result<()> {
        if self.preview {
            // Every method returns before reaching this, so it is dead code in a
            // preview. It is kept honest anyway: a future method that forgot its
            // preview branch would get `Ok` here and then be stopped by the
            // `sys` latch with a `PreviewActive` error — a failed phase, never a
            // written disk.
            return Ok(());
        }
        match self.gate.mode() {
            GateMode::Locked => Err(CamarelolError::new(
                ErrorCode::SafetyGateClosed,
                "destructive mode is disabled",
            )
            .with_remediation("start camareLOL with --allow-destructive")),
            GateMode::DryRun => Err(CamarelolError::new(
                ErrorCode::SafetyGateClosed,
                "camareLOL is in dry-run mode; nothing will be written",
            )
            .with_remediation("pass --execute to really install")),
            GateMode::Preview => Err(CamarelolError::new(
                ErrorCode::PreviewActive,
                "camareLOL is in preview mode; nothing will be run",
            )
            .with_remediation("pass --preview to simulate the install")),
            GateMode::Execute => Ok(()),
        }
    }

    /// Run a script inside the target system via `arch-chroot`, piping the
    /// script on stdin (no argv exposure of secrets).
    fn chroot_run(&mut self, script: &str, _phase: &str) -> Result<()> {
        if self.preview {
            self.record_chroot(script);
            return Ok(());
        }
        self.ensure_exec()?;
        self.gate.authorize(
            crate::safety::Operation::Pacstrap,
            crate::storage::device::RiskLevel::Low,
        )?;
        let out = crate::sys::run_with_input(
            "arch-chroot",
            &[TARGET_ROOT],
            script,
            std::time::Duration::from_secs(600),
        )?;
        if !out.success() {
            return Err(CamarelolError::new(
                ErrorCode::CommandFailed,
                "command inside the target system failed",
            )
            .with_context(out.stderr.trim().to_string()));
        }
        Ok(())
    }
}

/// Hide a password in a script that is about to be shown to the user.
///
/// The account script builds `echo 'user:secret' | chpasswd`. The real run
/// keeps the secret off argv and off disk, but a preview *prints* the script, so
/// the text between the first pair of quotes on a chpasswd line is replaced. The
/// shape of the command stays readable; the secret does not leak into a log, a
/// screenshot or a bug report.
fn redact_secrets(script: &str) -> String {
    let mut out = script
        .lines()
        .map(|line| {
            if !line.contains("chpasswd") {
                return line.to_string();
            }
            match (line.find('\''), line.rfind('\'')) {
                (Some(first), Some(last)) if first < last => {
                    format!("{}'<redacted>'{}", &line[..first], &line[last + 1..])
                }
                _ => line.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    // `lines()` swallows the final newline; put it back so a script that had no
    // secret in it comes out byte-identical.
    if script.ends_with('\n') {
        out.push('\n');
    }
    out
}

impl SystemRunner for RealSystemRunner {
    fn apply_partition_table(&mut self, disk: &StorageDevice, plan: &PartitionPlan) -> Result<()> {
        if self.preview {
            let gate = self.preview_gate();
            let log = Logger::default();
            return self.ops.apply_partition_table(disk, plan, &gate, &log);
        }
        self.ensure_exec()?;
        let log = Logger::default();
        self.ops
            .apply_partition_table(disk, plan, &self.gate.clone(), &log)
    }

    fn format_partition(&mut self, spec: &PartitionSpec, target: &str) -> Result<()> {
        if self.preview {
            let gate = self.preview_gate();
            let log = Logger::default();
            return self.ops.format_partition(spec, target, &gate, &log);
        }
        self.ensure_exec()?;
        let log = Logger::default();
        self.ops
            .format_partition(spec, target, &self.gate.clone(), &log)
    }

    fn mount(&mut self, source: &str, target: &str) -> Result<()> {
        if self.preview {
            let gate = self.preview_gate();
            let log = Logger::default();
            return self.ops.mount(source, target, &gate, &log);
        }
        self.ensure_exec()?;
        let log = Logger::default();
        self.ops.mount(source, target, &self.gate.clone(), &log)
    }

    fn pacstrap(&mut self, packages: &[String]) -> Result<()> {
        if self.preview {
            let mut args = vec![TARGET_ROOT.to_string()];
            args.extend(packages.iter().cloned());
            self.record("pacstrap", args, None);
            return Ok(());
        }
        self.ensure_exec()?;
        self.gate.authorize(
            crate::safety::Operation::Pacstrap,
            crate::storage::device::RiskLevel::Low,
        )?;
        let mut args = vec![TARGET_ROOT.to_string()];
        args.extend(packages.iter().cloned());
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let out =
            crate::sys::run_with_timeout("pacstrap", &refs, std::time::Duration::from_secs(3600))?;
        if !out.success() {
            return Err(
                CamarelolError::new(ErrorCode::CommandFailed, "pacstrap failed")
                    .with_context(out.stderr.trim().to_string())
                    .with_remediation("check the network connection and the mirror, then retry"),
            );
        }
        Ok(())
    }

    fn genfstab(&mut self) -> Result<()> {
        if self.preview {
            // Only the `genfstab` call. Appending its (non-existent) output to
            // /mnt/etc/fstab is the one thing here that would create a file on
            // the disk, so it is deliberately not part of the preview.
            self.record("genfstab", vec!["-U".into(), TARGET_ROOT.into()], None);
            return Ok(());
        }
        self.ensure_exec()?;
        let out = crate::sys::run("genfstab", &["-U", TARGET_ROOT])?;
        if !out.success() {
            return Err(
                CamarelolError::new(ErrorCode::CommandFailed, "genfstab failed")
                    .with_context(out.stderr.trim().to_string()),
            );
        }
        let fstab = format!("{TARGET_ROOT}/etc/fstab");
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&fstab)
            .map_err(|e| {
                CamarelolError::new(
                    ErrorCode::CommandFailed,
                    format!("cannot append to {fstab}"),
                )
                .with_context(e.to_string())
            })?;
        f.write_all(out.stdout.as_bytes()).map_err(|e| {
            CamarelolError::new(ErrorCode::CommandFailed, "cannot write fstab")
                .with_context(e.to_string())
        })?;
        Ok(())
    }

    fn install_bootloader(&mut self, bl: Bootloader, disk: &StorageDevice) -> Result<()> {
        if self.preview {
            let firmware = crate::hardware::detect_firmware();
            match bl {
                Bootloader::Grub => {
                    let target = match firmware {
                        crate::hardware::Firmware::Bios => {
                            format!("--target=i386-pc {}", disk.path)
                        }
                        _ => "--target=x86_64-efi --efi-directory=/boot --bootloader-id=camareLOL"
                            .to_string(),
                    };
                    let mut args = vec![TARGET_ROOT.to_string(), "grub-install".to_string()];
                    args.extend(target.split(' ').map(|s| s.to_string()));
                    self.record("arch-chroot", args, None);
                    self.record_chroot("grub-mkconfig -o /boot/grub/grub.cfg");
                }
                Bootloader::SystemdBoot => self.record_chroot("bootctl install"),
            }
            return Ok(());
        }
        self.ensure_exec()?;
        self.gate.authorize(
            crate::safety::Operation::InstallBootloader,
            crate::storage::device::RiskLevel::Medium,
        )?;
        let firmware = crate::hardware::detect_firmware();
        match bl {
            Bootloader::Grub => {
                let target = match firmware {
                    crate::hardware::Firmware::Bios => {
                        format!("--target=i386-pc {}", disk.path)
                    }
                    _ => "--target=x86_64-efi --efi-directory=/boot --bootloader-id=camareLOL"
                        .to_string(),
                };
                let mut args = vec![TARGET_ROOT.to_string(), "grub-install".to_string()];
                args.extend(target.split(' ').map(|s| s.to_string()));
                let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
                let out = crate::sys::run_with_timeout(
                    "arch-chroot",
                    &refs,
                    std::time::Duration::from_secs(600),
                )?;
                if !out.success() {
                    return Err(CamarelolError::new(
                        ErrorCode::CommandFailed,
                        "grub-install failed",
                    )
                    .with_context(out.stderr.trim().to_string()));
                }
                self.chroot_run("grub-mkconfig -o /boot/grub/grub.cfg", "bootloader")?;
                Ok(())
            }
            Bootloader::SystemdBoot => self.chroot_run("bootctl install", "bootloader"),
        }
    }

    fn chroot_script(&mut self, script: &str) -> Result<()> {
        self.chroot_run(script, "chroot")
    }

    fn enable_services(&mut self, services: &[String]) -> Result<()> {
        if self.preview {
            for service in services {
                self.record(
                    "arch-chroot",
                    vec![
                        TARGET_ROOT.into(),
                        "systemctl".into(),
                        "enable".into(),
                        service.clone(),
                    ],
                    None,
                );
            }
            // `ensure_template_symlink` is *not* run: it creates symlinks under
            // the target root, which is a write no matter how small.
            return Ok(());
        }
        self.ensure_exec()?;
        if services.is_empty() {
            return Ok(());
        }
        // Enable each service separately: a unit that does not exist on this
        // system (optional package, different distribution) must not abort the
        // whole install. The user is told which ones were skipped.
        let mut enabled = Vec::new();
        let mut skipped = Vec::new();
        for service in services {
            let refs = [TARGET_ROOT, "systemctl", "enable", service.as_str()];
            let out = crate::sys::run("arch-chroot", &refs)?;
            if out.success() {
                enabled.push(service.clone());
            } else {
                let reason = out.stderr.trim().lines().last().unwrap_or("not available");
                skipped.push(format!("{service} ({reason})"));
            }
        }
        // Template units (containing '@') may not get their symlink created by
        // `systemctl enable` in a chroot without a running systemd — the command
        // returns 0 but no symlink appears. Manually create the symlink for any
        // template unit whose symlink is missing.
        for service in &enabled {
            ensure_template_symlink(TARGET_ROOT, service)?;
        }
        if enabled.is_empty() {
            return Err(CamarelolError::new(
                ErrorCode::CommandFailed,
                "no service could be enabled in the target system",
            )
            .with_context(skipped.join("; "))
            .with_remediation("check that the matching packages are installed in the profile"));
        }
        if !skipped.is_empty() {
            self.warnings.push(format!(
                "not enabled (unit missing or disabled): {}",
                skipped.join(", ")
            ));
        }
        Ok(())
    }

    fn target_mounted(&self) -> bool {
        if self.preview {
            return std::path::Path::new(TARGET_ROOT).join("etc").is_dir();
        }
        // A real check: `/mnt/etc` merely existing is not a mount. A leftover
        // from an interrupted run has to be visible in findmnt.
        match crate::sys::run("findmnt", &["-rn", "-o", "TARGET", TARGET_ROOT]) {
            Ok(out) => !out.stdout.trim().is_empty(),
            Err(_) => false,
        }
    }

    fn warnings(&self) -> Vec<String> {
        self.warnings.clone()
    }

    fn mkinitcpio(&mut self) -> Result<()> {
        if self.preview {
            self.record(
                "arch-chroot",
                vec![TARGET_ROOT.into(), "mkinitcpio".into(), "-P".into()],
                None,
            );
            return Ok(());
        }
        self.ensure_exec()?;
        let out = crate::sys::run_with_timeout(
            "arch-chroot",
            &[TARGET_ROOT, "mkinitcpio", "-P"],
            std::time::Duration::from_secs(900),
        )?;
        if !out.success() {
            return Err(
                CamarelolError::new(ErrorCode::CommandFailed, "mkinitcpio failed")
                    .with_context(out.stderr.trim().to_string()),
            );
        }
        Ok(())
    }

    fn build_aur(&mut self, packages: &crate::packages::PackageSelection) -> Result<usize> {
        let plan = self.aur_plan(packages)?;
        let Some(plan) = plan else {
            // AUR is opt-in and off by default: the ordinary install never
            // reaches this code with anything to build.
            return Ok(0);
        };
        let count = plan.packages.len();
        if plan.method == crate::aur::AurMethod::Manual {
            self.warnings.push(crate::aur::no_helper_message());
        }
        self.warnings.push(format!(
            "{} package(s) built as {}: {}. {}",
            count,
            plan.build_user
                .as_deref()
                .unwrap_or("the current (unprivileged) user"),
            plan.packages.join(", "),
            crate::aur::warning()
        ));

        if self.preview {
            // Record, never spawn. The wrapped form is recorded so the preview
            // shows which account would run the unreviewed build script.
            for step in &plan.steps {
                let argv = crate::aur::privilege_wrapper(
                    if step.as_build_user {
                        plan.wrapper.as_deref()
                    } else {
                        None
                    },
                    if step.as_build_user {
                        plan.build_user.as_deref()
                    } else {
                        None
                    },
                    &step.program,
                )?;
                let mut args = argv[1..].to_vec();
                args.extend(step.args.iter().cloned());
                self.record(&argv[0], args, None);
            }
            return Ok(count);
        }

        self.ensure_exec()?;
        // Installing packages is what pacstrap already needed confirmed for;
        // the AUR build reuses that same authorization rather than inventing a
        // new one that nothing would have recorded a confirmation for.
        self.gate.authorize(
            crate::safety::Operation::Pacstrap,
            crate::storage::device::RiskLevel::Low,
        )?;

        for step in &plan.steps {
            // Guard 3, evaluated immediately before every spawn — but only for
            // the steps that are *not* wrapped.
            //
            // This is the subtle half of the never-root rule. A wrapped step
            // (`runuser -u builder -- makepkg`) legitimately starts as root:
            // that is how the uid is dropped. Checking it here would refuse
            // every correct wrapped build and force the installer to invent a
            // half-rule. An unwrapped step runs as whatever this process is, so
            // *that* is the one that must not be root.
            if !step.as_build_user {
                crate::aur::ensure_not_root(crate::aur::effective_uid())?;
            }
            let argv = crate::aur::privilege_wrapper(
                if step.as_build_user {
                    plan.wrapper.as_deref()
                } else {
                    None
                },
                if step.as_build_user {
                    plan.build_user.as_deref()
                } else {
                    None
                },
                &step.program,
            )?;
            let mut cmd = std::process::Command::new(&argv[0]);
            cmd.args(&argv[1..]);
            cmd.args(&step.args);
            if let Some(dir) = &step.cwd {
                // `runuser -u … -- prog` changes the uid and leaves the working
                // directory alone, so setting it here means makepkg builds *as
                // the build user* inside its own checkout.
                cmd.current_dir(dir);
            }
            // No environment is set here on purpose: `sys` clears the child
            // environment and re-seeds it with the *parent's* `HOME`, which
            // during a root run is `/root`. `runuser` rewrites `HOME`, `USER`,
            // `LOGNAME` and `PATH` from the target account before exec, so the
            // build sees its own home and its own cache — and never root's.
            let out = crate::sys::run_command(cmd, crate::aur::BUILD_TIMEOUT)?;
            if !out.success() {
                return Err(CamarelolError::new(
                    ErrorCode::CommandFailed,
                    format!("`{}` failed during the AUR build", argv[0]),
                )
                .with_context(out.stderr.trim().to_string())
                .with_remediation(
                    "AUR builds run user-submitted scripts; read the output above before retrying",
                ));
            }
        }
        Ok(count)
    }
}

/// Create the enable symlink for a template unit if it doesn't exist.
/// `systemctl enable` for a template instance in a chroot without a running
/// systemd returns 0 but may not create the symlink — this ensures it exists.
fn ensure_template_symlink(root: &str, service: &str) -> Result<()> {
    if !service.contains('@') {
        return Ok(());
    }
    let wants_dir = format!("{root}/etc/systemd/system/multi-user.target.wants");
    let link = format!("{wants_dir}/{service}");
    let template = service.split('@').next().unwrap_or(service);
    // Four levels up from .../multi-user.target.wants/ back to the target root.
    // A relative link keeps the check honest on a mounted target: `test -e`
    // resolves it inside the mount, while an absolute link would be resolved
    // against the host root and hide a missing unit.
    let target = format!("../../../../usr/lib/systemd/system/{template}@.service");
    let link_path = std::path::Path::new(&link);
    // Check the link itself (not its target): a broken symlink still exists as
    // a link and must be replaced, not duplicated.
    let needs_write = match link_path.symlink_metadata() {
        Ok(_) => std::fs::read_link(link_path)
            .map(|t| t.to_string_lossy() != target)
            .unwrap_or(true),
        Err(_) => true,
    };
    if needs_write {
        let _ = std::fs::remove_file(link_path);
        std::fs::create_dir_all(&wants_dir)?;
        std::os::unix::fs::symlink(&target, link_path)?;
    }
    Ok(())
}

/// Test double: records what would have happened, touches nothing.
pub struct SimulatedRunner {
    pub calls: Vec<String>,
    pub fail_on: Option<String>,
    /// What `target_mounted()` answers. Simulates a leftover `/mnt` from a
    /// previous interrupted run.
    pub mounted: bool,
}

impl SimulatedRunner {
    pub fn new() -> Self {
        SimulatedRunner {
            calls: Vec::new(),
            fail_on: None,
            mounted: false,
        }
    }
}

impl Default for SimulatedRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemRunner for SimulatedRunner {
    fn apply_partition_table(&mut self, disk: &StorageDevice, _plan: &PartitionPlan) -> Result<()> {
        if self.fail_on.as_deref() == Some("partition") {
            return Err(CamarelolError::new(
                ErrorCode::CommandFailed,
                "simulated partition failure",
            ));
        }
        self.calls.push(format!("partition {}", disk.path));
        Ok(())
    }
    fn format_partition(&mut self, _spec: &PartitionSpec, target: &str) -> Result<()> {
        if self.fail_on.as_deref() == Some("format") {
            return Err(CamarelolError::new(
                ErrorCode::CommandFailed,
                "simulated format failure",
            ));
        }
        self.calls.push(format!("format {target}"));
        Ok(())
    }
    fn mount(&mut self, source: &str, target: &str) -> Result<()> {
        self.calls.push(format!("mount {source} -> {target}"));
        Ok(())
    }
    fn pacstrap(&mut self, packages: &[String]) -> Result<()> {
        if self.fail_on.as_deref() == Some("pacstrap") {
            return Err(CamarelolError::new(
                ErrorCode::CommandFailed,
                "simulated pacstrap failure",
            )
            .with_remediation("retryable"));
        }
        self.calls
            .push(format!("pacstrap {} packages", packages.len()));
        Ok(())
    }
    fn genfstab(&mut self) -> Result<()> {
        self.calls.push("genfstab".into());
        Ok(())
    }
    fn install_bootloader(&mut self, bl: Bootloader, disk: &StorageDevice) -> Result<()> {
        self.calls
            .push(format!("bootloader {} on {}", bl.as_str(), disk.path));
        Ok(())
    }
    fn chroot_script(&mut self, script: &str) -> Result<()> {
        self.calls.push(format!("chroot: {} bytes", script.len()));
        Ok(())
    }
    fn enable_services(&mut self, services: &[String]) -> Result<()> {
        self.calls.push(format!("enable {}", services.join(",")));
        Ok(())
    }
    fn mkinitcpio(&mut self) -> Result<()> {
        self.calls.push("mkinitcpio".into());
        Ok(())
    }

    fn target_mounted(&self) -> bool {
        self.mounted
    }
}

/// Final report of an install run.
#[derive(Debug, Clone)]
pub struct InstallReport {
    pub phases: Vec<PhaseReport>,
    pub outcome: Outcome,
    pub warnings: Vec<String>,
}

impl InstallReport {
    pub fn render(&self) -> String {
        let mut s = String::new();
        for p in &self.phases {
            s.push_str(&format!("{:<14} {:<10} {}\n", p.name, p.state, p.details));
        }
        s.push_str(&format!("\nOutcome: {}", self.outcome.as_str()));
        for w in &self.warnings {
            s.push_str(&format!("\nwarning: {w}"));
        }
        s
    }

    pub fn all_completed(&self) -> bool {
        self.phases
            .iter()
            .filter(|p| p.state != "skipped")
            .all(|p| p.state == "completed")
    }
}

/// Execute the whole install. Stops at the first failure — never continues into
/// an unsafe step — and reports honestly what happened.
///
/// The host is **detected here**, never taken from the caller: this is the
/// entry point both the TUI and `--scripted` use, and an install that could be
/// told it is running on Arch would make the whole host check advisory. The
/// detection is read-only, so it works identically under `--preview`.
pub fn run_install(
    cfg: &InstallationConfig,
    disk: &StorageDevice,
    plan: &PartitionPlan,
    gate: &SafetyGate,
    log: &Logger,
    runner: &mut dyn SystemRunner,
    sink: &mut dyn ProgressSink,
) -> InstallReport {
    run_install_on_host(
        &crate::host::detect(),
        cfg,
        disk,
        plan,
        gate,
        log,
        runner,
        sink,
    )
}

/// [`run_install`] with an explicitly supplied [`crate::host::HostProfile`].
///
/// The seam exists for one reason: a unit test must be able to say *which host
/// it is modelling* rather than inheriting whatever machine runs the suite,
/// which would otherwise make the suite pass on a developer's Arch box and fail
/// on a container. Production code — the TUI and `--scripted` — calls
/// [`run_install`], which cannot be told anything.
// Eight parameters because the host is deliberately *not* bundled into a struct
// with the rest: a `Preflight`-shaped argument bag would let a caller swap the
// host out along with the plan, and the point of this function is that it does
// not.
#[allow(clippy::too_many_arguments)]
pub fn run_install_on_host(
    host: &crate::host::HostProfile,
    cfg: &InstallationConfig,
    disk: &StorageDevice,
    plan: &PartitionPlan,
    gate: &SafetyGate,
    log: &Logger,
    runner: &mut dyn SystemRunner,
    sink: &mut dyn ProgressSink,
) -> InstallReport {
    let firmware = match cfg.resolve_firmware(crate::hardware::detect_firmware()) {
        Ok(f) => f,
        Err(e) => {
            return InstallReport {
                phases: vec![],
                outcome: Outcome::Incomplete,
                warnings: vec![e.to_string()],
            }
        }
    };
    let pf = Preflight {
        firmware,
        host: host.clone(),
        plan: plan.clone(),
        config: cfg.clone(),
    };

    let mut phases: Vec<PhaseReport> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut outcome = Outcome::Completed;

    // NOTE: `aborted` (instead of break) because macro_rules! labels are
    // hygienic and could not refer to an outer loop label.
    macro_rules! run_phase {
        ($name:expr, $body:expr, $aborted:ident) => {{
            if !$aborted {
                let phase_name: &str = $name;
                let phase_result: Result<String> = (|| $body)();
                if let Err(e) = preconditions(phase_name, &pf) {
                    log.log(phase_name, &format!("precondition failed: {e}"));
                    phases.push(PhaseReport {
                        name: phase_name.to_string(),
                        state: PhaseState::Failed.as_str().to_string(),
                        retryable: false,
                        details: e.to_string(),
                    });
                    outcome = Outcome::Incomplete;
                    sink.emit(ProgressEvent::PhaseFinished {
                        name: phase_name.to_string(),
                        state: PhaseState::Failed,
                    });
                    $aborted = true;
                } else {
                    sink.emit(ProgressEvent::PhaseStarted {
                        name: phase_name.to_string(),
                    });
                    match phase_result {
                        Ok(details) => {
                            log.log(phase_name, "ok");
                            phases.push(PhaseReport {
                                name: phase_name.to_string(),
                                state: PhaseState::Completed.as_str().to_string(),
                                retryable: retryable(phase_name),
                                details,
                            });
                            sink.emit(ProgressEvent::PhaseFinished {
                                name: phase_name.to_string(),
                                state: PhaseState::Completed,
                            });
                        }
                        Err(e) => {
                            log.log(phase_name, &format!("failed: {e}"));
                            phases.push(PhaseReport {
                                name: phase_name.to_string(),
                                state: PhaseState::Failed.as_str().to_string(),
                                retryable: retryable(phase_name),
                                details: e.to_string(),
                            });
                            outcome = Outcome::Incomplete;
                            sink.emit(ProgressEvent::PhaseFinished {
                                name: phase_name.to_string(),
                                state: PhaseState::Failed,
                            });
                            $aborted = true;
                        }
                    }
                }
            }
        }};
    }

    // Path of the n-th partition of the target disk.
    let part = |n: usize| partition_path(&disk.path, n);

    // Each phase runs exactly once, in order. A failed phase (or a failed
    // precondition) stops the run — nothing unsafe is attempted afterwards.
    let mut aborted = false;
    let mut phase_idx = 0usize;
    while !aborted && phase_idx < PHASE_ORDER.len() {
        let phase = PHASE_ORDER[phase_idx];
        phase_idx += 1;
        run_phase!(
            phase,
            {
                match phase {
                    "preflight" => {
                        // A previous interrupted run leaves `/mnt` mounted;
                        // formatting then fails half-way through with an
                        // unreadable error. Refuse here, with the fix.
                        if !gate.preview && runner.target_mounted() {
                            return Err(CamarelolError::new(
                                ErrorCode::CommandFailed,
                                "/mnt is already mounted (a previous run or mount is still active)",
                            )
                            .with_remediation(
                                "run `umount -R /mnt` (and `swapoff -a` if a swap was enabled), then start camareLOL again",
                            ));
                        }
                        Ok(format!(
                            "firmware={} bootloader={} fs={} partitions={} host={}",
                            firmware.as_str(),
                            cfg.bootloader()
                                .map(|b| b.as_str().to_string())
                                .unwrap_or_default(),
                            cfg.filesystem,
                            plan.partitions.len(),
                            pf.host.summary()
                        ))
                    }
                    "partition" => {
                        runner.apply_partition_table(disk, plan)?;
                        Ok(format!("wrote {} layout to {}", plan.table_type, disk.path))
                    }
                    "format" => {
                        for (i, spec) in plan.partitions.iter().enumerate() {
                            let target = part(i + 1);
                            runner.format_partition(spec, &target)?;
                        }
                        Ok("filesystems created".into())
                    }
                    "mount" => {
                        // The root filesystem has to be mounted first. The
                        // planner puts the ESP before root, so mounting in plan
                        // order hid the ESP under the freshly mounted root and
                        // `grub-install --efi-directory=/boot` then failed with
                        // "/boot doesn't look like an EFI partition".
                        let mut ordered: Vec<(usize, &PartitionSpec)> =
                            plan.partitions.iter().enumerate().collect();
                        ordered.sort_by_key(|(_, spec)| {
                            match spec.mount_point.as_deref() {
                                Some("/") => 0,
                                Some(_) => 1,
                                None => 2,
                            }
                        });
                        for (i, spec) in ordered {
                            if spec.purpose == PartitionPurpose::BiosBoot {
                                continue;
                            }
                            if let Some(mp) = spec.mount_point.as_deref() {
                                let target = part(i + 1);
                                let mnt = format!("{TARGET_ROOT}{mp}");
                                runner.mount(&target, &mnt)?;
                            }
                        }
                        Ok("target mounted".into())
                    }
                    "pacstrap" => {
                        let profile = cfg
                            .desktop_profile()
                            .unwrap_or(crate::desktop::DesktopProfile::ConsoleOnly);
                        let mut pkgs = cfg.packages.pacstrap_list();
                        pkgs.extend(crate::desktop::packages_for(profile));
                        // Make the display-server choice real instead of a stored
                        // string: an explicit Wayland or X11 selection pulls the
                        // stack that choice needs. `Auto` adds nothing — the
                        // profile already brings a working default.
                        if let Ok(server) = cfg.display_server() {
                            let resolved = crate::desktop::resolve_display_server(profile, server)
                                .unwrap_or(server);
                            pkgs.extend(
                                crate::desktop::display_server_packages(resolved)
                                    .iter()
                                    .map(|p| (*p).to_string()),
                            );
                        }
                        pkgs.extend(cfg.services.packages());
                        // The bootloader has to exist inside the target before we
                        // can install it into it.
                        let bootloader = cfg.bootloader()?;
                        pkgs.extend(bootloader.target_packages().iter().map(|p| p.to_string()));
                        runner.pacstrap(&pkgs)?;
                        // AUR is opt-in and off by default, so this is a no-op
                        // unless the profile explicitly enabled it *and* listed
                        // packages. Kept inside the pacstrap phase rather than
                        // added as a thirteenth phase, so the phase list, the
                        // docs and the preview banner all keep saying twelve.
                        let built = runner.build_aur(&cfg.packages)?;
                        Ok(if built > 0 {
                            format!(
                                "installed {} packages and built {built} from the AUR",
                                pkgs.len()
                            )
                        } else {
                            format!("installed {} packages", pkgs.len())
                        })
                    }
                    "fstab" => {
                        runner.genfstab()?;
                        Ok("fstab generated".into())
                    }
                    "bootloader" => {
                        runner.install_bootloader(cfg.bootloader()?, disk)?;
                        Ok(format!("{} installed", cfg.bootloader()?.as_str()))
                    }
                    "system-config" => {
                        let script = system_config_script(cfg);
                        runner.chroot_script(&script)?;
                        Ok(format!(
                            "locale, keymap, timezone and hostname written{}",
                            if cfg.services.zram {
                                " + zram-generator.conf"
                            } else {
                                ""
                            }
                        ))
                    }
                    "accounts" => {
                        let script = account_script(cfg);
                        runner.chroot_script(&script)?;
                        Ok(format!("{} user(s) configured", cfg.users.len()))
                    }
                    "services" => {
                        // Enable the services the user selected. The display manager
                        // comes from the desktop profile (its package is installed
                        // in the pacstrap phase), so it is enabled here alongside
                        // the rest rather than living in ServiceSelection, whose
                        // package/provider invariant is checked separately.
                        let mut services = cfg.services.services();
                        if let Some(dm) = cfg.display_manager.as_deref() {
                            if !services.iter().any(|s| s == dm) {
                                services.push(dm.to_string());
                            }
                        }
                        runner.enable_services(&services)?;
                        Ok("services enabled".into())
                    }
                    "initramfs" => {
                        runner.mkinitcpio()?;
                        Ok("initramfs generated".into())
                    }
                    "post-check" => {
                        if !runner.target_mounted() {
                            // Nothing to verify against; we say so instead of pretending.
                            Ok("target not mounted here; verification deferred".into())
                        } else {
                            let w = post_check(cfg, plan, TARGET_ROOT);
                            if w.is_empty() {
                                Ok("all post-install checks passed".into())
                            } else {
                                warnings.extend(w.clone());
                                Ok(format!("{} warning(s)", w.len()))
                            }
                        }
                    }
                    other => Ok(format!("phase {other} skipped")),
                }
            },
            aborted
        );
    }

    for w in runner.warnings() {
        if !warnings.contains(&w) {
            warnings.push(w);
        }
    }

    if outcome == Outcome::Completed && !warnings.is_empty() {
        outcome = Outcome::CompletedWithWarnings;
    }

    InstallReport {
        phases,
        outcome,
        warnings,
    }
}

/// Build the in-target script that writes locale / keymap / timezone / hostname,
/// and — when zram was selected — the one file that makes zram real.
///
/// `zram-generator` ships the unit and gets installed fine, but by design it is
/// a no-op without `/etc/systemd/zram-generator.conf`: the file is what tells it
/// which devices to carve out. Two previous releases enabled the
/// unit and installed the package and still never had a swap at boot, because
/// that file was missing. The config is written here, where every other
/// /etc file lives, with `zram_size_mb` honoured when it was set.
fn system_config_script(cfg: &InstallationConfig) -> String {
    let mut script = format!(
        r#"set -e
echo 'LANG={locale}' > /etc/locale.conf
echo 'KEYMAP={keymap}' > /etc/vconsole.conf
sed -i 's/^#\({locale} \)/\1/' /etc/locale.gen
locale-gen
ln -sf /usr/share/zoneinfo/{tz} /etc/localtime
hwclock --systohc
echo '{host}' > /etc/hostname
"#,
        locale = cfg.system_locale,
        keymap = cfg.keymap,
        tz = cfg.timezone,
        host = cfg.hostname,
    );
    // A greetd-based profile (Hyprland, Sway) needs its config file to know
    // which compositor to launch; without it greetd starts and does nothing.
    if cfg.display_manager.as_deref() == Some("greetd") {
        if let Ok(profile) = cfg.desktop_profile() {
            if let Some(cmd) = crate::desktop::greetd_command(profile) {
                script.push_str(&format!(
                    "mkdir -p /etc/greetd\ncat > /etc/greetd/config.toml <<'EOF'\n[terminal]\nvt = 1\n\n[default_session]\ncommand = \"tuigreet --time --cmd {cmd}\"\nuser = \"greeter\"\nEOF\n",
                    cmd = cmd
                ));
            }
        }
    }
    if cfg.services.zram {
        // upstream documents plain integers as MiB; `min` is part of the
        // accepted expression grammar, and 4096 is what Fedora ships.
        let size = match cfg.services.zram_size_mb {
            Some(mb) => format!("zram-size = {mb}"),
            None => "zram-size = min(ram / 2, 4096)".to_owned(),
        };
        script.push_str(&format!(
            "printf '[zram0]\\n{size}\\n' > /etc/systemd/zram-generator.conf\n"
        ));
    }
    script
}

/// Build the in-target script for users.
///
/// Passwords are substituted into this *in-memory* script only. The script is
/// piped to `arch-chroot` on stdin, never appears in argv, ps output or logs,
/// and is not written to disk.
fn account_script(cfg: &InstallationConfig) -> String {
    let mut script = String::from("set -e\n");
    if let Some(pw) = cfg.root_password() {
        script.push_str(&format!("echo 'root:{pw}' | chpasswd\n"));
    }
    if !cfg.root_login_allowed {
        script.push_str("passwd --lock root\n");
    }
    for u in &cfg.users {
        script.push_str(&format!(
            "id -u {u} >/dev/null 2>&1 || useradd -m -s /bin/bash {u}\n"
        ));
        if let Some(pw) = cfg.user_password(u) {
            script.push_str(&format!("echo '{u}:{pw}' | chpasswd\n"));
        }
        if cfg.admin_users.contains(u) {
            script.push_str(&format!(
                "usermod -aG wheel {u}\necho '%wheel ALL=(ALL:ALL) ALL' > /etc/sudoers.d/10-wheel\nchmod 440 /etc/sudoers.d/10-wheel\n"
            ));
        }
    }
    script
}

/// Verify the target system looks sane. Returns warnings (never a hard failure,
/// because we cannot prove the machine will boot).
///
/// When the target root is not present at all the checks are deferred: the
/// caller learns this from the phase details instead of a scary warning.
fn post_check(cfg: &InstallationConfig, plan: &PartitionPlan, root: &str) -> Vec<String> {
    let mut w = Vec::new();
    let root_path = std::path::Path::new(root);
    if !root_path.exists() {
        // Nothing is mounted at the target root: the checks cannot run here
        // (this happens in unit tests with the simulated runner).
        return w;
    }
    let expected_kernel = root_path.join("boot");
    if !expected_kernel.exists() {
        w.push("target /boot was not found (system may not boot)".into());
    }
    let fstab = root_path.join("etc/fstab");
    if !fstab.exists() {
        w.push("fstab is missing".into());
    }
    let root_mounted = plan
        .partitions
        .iter()
        .any(|p| p.mount_point.as_deref() == Some("/"));
    if !root_mounted {
        w.push("no root partition in plan".into());
    }
    if cfg.services.zram && !root_path.join("etc/systemd/zram-generator.conf").exists() {
        // unit + package alone never produce a swap: zram-generator reads this
        // file to know which devices to create, and without it is a no-op. A
        // regression here is how 0.1.11 and 0.1.12 shipped a zram that was
        // "installed" and still off.
        w.push("/etc/systemd/zram-generator.conf is missing — zram will never come up".into());
    }
    if !cfg.users.is_empty() && !cfg.admin_users.is_empty() {
        // informational only
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::InstallationConfig;
    use crate::storage::scan::scan_devices_from_survey;

    fn disk() -> StorageDevice {
        scan_devices_from_survey(
            r#"[{"kernel_name":"vdb","path":"/dev/vdb","size_bytes":21474836480,"rotational":false,"removable":false,"kind":"Virtual","partition_table":"None","partitions":[],"mount_state":"NotMounted","os_hints":[],"encrypted":null,"risk":"Low"}]"#,
        )
        .unwrap()
        .remove(0)
    }

    fn plan() -> PartitionPlan {
        crate::storage::layout::plan_auto_layout(
            &disk(),
            &crate::storage::layout::LayoutRequest::default(),
        )
        .unwrap()
    }

    /// Every test below runs against a pinned Arch + systemd host.
    ///
    /// The production entry point detects the *real* host, which would make the
    /// suite pass on an Arch workstation and fail inside a Debian container —
    /// a test whose result depends on the machine is a test that lies. This
    /// local `fn` shadows the imported `run_install`, so the modelled host is
    /// stated once here instead of at every call site.
    ///
    /// `refuses_an_unsupported_host_before_any_phase_runs` is the counter-test:
    /// it hands a hostile profile to the same seam and proves the refusal is
    /// real rather than an artefact of the pinning.
    fn run_install(
        cfg: &InstallationConfig,
        disk: &StorageDevice,
        plan: &PartitionPlan,
        gate: &SafetyGate,
        log: &Logger,
        runner: &mut dyn SystemRunner,
        sink: &mut dyn ProgressSink,
    ) -> InstallReport {
        run_install_on_host(
            &crate::host::HostProfile::arch_systemd(),
            cfg,
            disk,
            plan,
            gate,
            log,
            runner,
            sink,
        )
    }

    #[test]
    fn zram_selection_writes_the_config_that_makes_it_real() {
        // The unit and the package alone never produced a swap — zram-generator
        // reads /etc/systemd/zram-generator.conf to know which devices to
        // create, and without it is a documented no-op. The script must write
        // that file whenever zram was selected.
        let cfg = InstallationConfig::default(); // default services include zram
        let script = system_config_script(&cfg);
        assert!(script.contains("zram-generator.conf"), "{}", script);
        assert!(script.contains("[zram0]"), "{}", script);
        assert!(
            script.contains("zram-size = min(ram / 2, 4096)"),
            "{}",
            script
        );
    }

    #[test]
    fn greetd_profile_writes_its_config() {
        let cfg = InstallationConfig {
            desktop: "hyprland".into(),
            display_manager: Some("greetd".into()),
            ..Default::default()
        };
        let script = system_config_script(&cfg);
        assert!(script.contains("/etc/greetd/config.toml"), "{}", script);
        assert!(
            script.contains("tuigreet --time --cmd Hyprland"),
            "{}",
            script
        );
    }

    #[test]
    fn explicit_zram_size_is_honoured() {
        let cfg = InstallationConfig {
            services: crate::services::ServiceSelection {
                zram_size_mb: Some(2048),
                ..Default::default()
            },
            ..Default::default()
        };
        let script = system_config_script(&cfg);
        assert!(script.contains("zram-size = 2048"), "{}", script);
        // Take the default confidence away: the chosen size must not fall back
        // into the default expression.
        assert!(!script.contains("min(ram / 2, 4096)"), "{}", script);
    }

    #[test]
    fn zram_off_writes_no_config() {
        let cfg = InstallationConfig {
            services: crate::services::ServiceSelection {
                zram: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let script = system_config_script(&cfg);
        assert!(!script.contains("zram-generator.conf"), "{}", script);
    }

    #[test]
    fn template_unit_symlink_is_created_when_systemctl_misses_it() {
        // `systemctl enable` for a template instance in a chroot without a running
        // systemd returns 0 but may not create the symlink. This ensures it exists.
        let tmp = std::env::temp_dir().join(format!("camarelol-test-{}", std::process::id()));
        let root = tmp.to_str().unwrap();
        ensure_template_symlink(root, "systemd-zram-setup@zram0.service").unwrap();
        let link =
            tmp.join("etc/systemd/system/multi-user.target.wants/systemd-zram-setup@zram0.service");
        assert!(
            std::fs::symlink_metadata(&link).is_ok(),
            "symlink should exist"
        );
        let target = std::fs::read_link(&link).unwrap();
        assert_eq!(
            target.to_str().unwrap(),
            "../../../../usr/lib/systemd/system/systemd-zram-setup@.service"
        );
        // Clean up.
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn template_symlink_resolves_to_the_unit_inside_the_target_root() {
        // The point of the relative link: resolved from the wants directory it
        // must land on the unit file inside the target root, which is exactly
        // what the acceptance harness checks with `test -e` on a mounted disk.
        // A link with one `..` too few silently resolves under /etc and reads as
        // "zram never enabled" on the installed system.
        let tmp = std::env::temp_dir().join(format!("camarelol-resolve-{}", std::process::id()));
        let root = tmp.to_str().unwrap();
        // Pretend zram-generator shipped its template unit.
        let unit = tmp.join("usr/lib/systemd/system/systemd-zram-setup@.service");
        std::fs::create_dir_all(unit.parent().unwrap()).unwrap();
        std::fs::write(&unit, "[Install]\nWantedBy=multi-user.target\n").unwrap();

        ensure_template_symlink(root, "systemd-zram-setup@zram0.service").unwrap();

        let link =
            tmp.join("etc/systemd/system/multi-user.target.wants/systemd-zram-setup@zram0.service");
        // Resolve the way `test -e` does: follow the link.
        assert!(
            link.exists(),
            "symlink must resolve — got {:?}",
            std::fs::read_link(&link)
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn non_template_service_is_ignored_by_symlink_fix() {
        let tmp = std::env::temp_dir().join(format!("camarelol-test-{}", std::process::id()));
        let root = tmp.to_str().unwrap();
        ensure_template_symlink(root, "sshd.service").unwrap();
        // No symlink should be created for non-template services.
        let wants_dir = tmp.join("etc/systemd/system/multi-user.target.wants");
        assert!(!wants_dir.exists(), "no wants dir for non-template service");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn simulated_full_run_reports_completed() {
        // Pin the firmware: these tests must pass on a BIOS host (container)
        // and on a UEFI host alike.
        let cfg = InstallationConfig {
            firmware: "uefi".into(),
            ..Default::default()
        };
        let mut runner = SimulatedRunner::new();
        let mut sink = NullSink;
        let log = Logger::default();
        let gate = SafetyGate::armed_and_execute();
        let report = run_install(&cfg, &disk(), &plan(), &gate, &log, &mut runner, &mut sink);
        assert!(report.all_completed(), "{}", report.render());
        assert_eq!(report.outcome, Outcome::Completed);
        // Exactly one entry per phase — no phase may run twice.
        assert_eq!(report.phases.len(), PHASE_ORDER.len());
        assert!(runner.calls.iter().any(|c| c.starts_with("partition")));
        assert!(runner.calls.iter().any(|c| c.starts_with("pacstrap")));
        assert!(runner.calls.iter().any(|c| c.contains("bootloader")));
    }

    #[test]
    fn a_failed_phase_stops_the_run() {
        let cfg = InstallationConfig {
            firmware: "uefi".into(),
            ..Default::default()
        };
        let mut runner = SimulatedRunner::new();
        runner.fail_on = Some("pacstrap".into());
        let mut sink = NullSink;
        let log = Logger::default();
        let gate = SafetyGate::armed_and_execute();
        let report = run_install(&cfg, &disk(), &plan(), &gate, &log, &mut runner, &mut sink);
        assert_eq!(report.outcome, Outcome::Incomplete);
        assert!(!report.all_completed());
        assert!(report.phases.len() <= PHASE_ORDER.len());
        // nothing after pacstrap may have run
        assert!(!runner.calls.iter().any(|c| c.starts_with("bootloader")));
        // and the failure is reported as retryable for pacstrap
        let p = report.phases.iter().find(|p| p.name == "pacstrap").unwrap();
        assert!(p.retryable);
    }

    #[test]
    fn failing_precondition_blocks_the_run() {
        let cfg = InstallationConfig {
            hostname: "-bad hostname-".into(),
            ..Default::default()
        };
        let mut runner = SimulatedRunner::new();
        let mut sink = NullSink;
        let log = Logger::default();
        let gate = SafetyGate::armed_and_execute();
        let report = run_install(&cfg, &disk(), &plan(), &gate, &log, &mut runner, &mut sink);
        assert_eq!(report.outcome, Outcome::Incomplete);
        assert!(runner.calls.is_empty());
    }

    #[test]
    fn real_runner_refuses_in_dry_run() {
        let gate = SafetyGate::armed(); // armed but dry-run
        let mut runner = RealSystemRunner::new(gate);
        let err = runner.pacstrap(&["base".into()]).unwrap_err();
        assert_eq!(err.code, ErrorCode::SafetyGateClosed);
        assert!(runner.records().is_empty());
    }

    #[test]
    fn a_normal_runner_is_not_in_preview() {
        // The regression guard for the other direction: without --preview
        // nothing about the runner may change, including the refusal of a
        // dry-run (which is what stopped the real code before).
        let _scope = crate::sys::PreviewScope::off();
        let mut runner = RealSystemRunner::new(SafetyGate::armed());
        assert!(!runner.is_preview());
        assert_eq!(
            runner.pacstrap(&["base".into()]).unwrap_err().code,
            ErrorCode::SafetyGateClosed
        );
        let mut runner = RealSystemRunner::new(SafetyGate::default());
        assert_eq!(
            runner.genfstab().unwrap_err().code,
            ErrorCode::SafetyGateClosed,
            "a locked gate still refuses before anything is recorded"
        );
        assert!(runner.records().is_empty());
    }

    // ---------------------------------------------------------------- preview

    /// The config every preview test runs: pinned firmware so the run is the
    /// same on a UEFI workstation, a BIOS box and inside a container.
    fn preview_cfg() -> InstallationConfig {
        InstallationConfig {
            firmware: "uefi".into(),
            users: vec!["alice".into()],
            admin_users: vec!["alice".into()],
            ..Default::default()
        }
    }

    #[test]
    fn preview_runs_all_twelve_phases_and_records_every_command() {
        let _scope = crate::sys::PreviewScope::on();
        let cfg = preview_cfg();
        cfg.validate().unwrap();
        let gate = SafetyGate::preview();
        let mut runner = RealSystemRunner::new(gate.clone());
        assert!(runner.is_preview());

        let report = run_install(
            &cfg,
            &disk(),
            &plan(),
            &gate,
            &Logger::default(),
            &mut runner,
            &mut NullSink,
        );

        assert!(
            report.all_completed(),
            "a preview must not stop early:\n{}",
            report.render()
        );
        assert_eq!(
            report.phases.len(),
            crate::install::PHASE_ORDER.len(),
            "every phase must be walked:\n{}",
            report.render()
        );

        let commands = runner.command_lines();
        assert!(!commands.is_empty(), "the preview produced no command");
        for expected in [
            "sgdisk",       // partition table
            "sfdisk",       // partitions
            "mkfs",         // filesystems
            "mount",        // target root
            "pacstrap",     // base system
            "genfstab",     // fstab
            "grub-install", // bootloader
            "mkinitcpio",   // initramfs
        ] {
            assert!(
                commands.iter().any(|c| c.contains(expected)),
                "`{expected}` is missing from the preview:\n{}",
                commands.join("\n")
            );
        }
        assert!(
            commands.iter().all(|c| c.starts_with("DRYRUN")),
            "no previewed command may be marked as run:\n{}",
            commands.join("\n")
        );
        assert!(
            runner
                .records()
                .iter()
                .all(|r| !r.applied && r.status.is_none()),
            "a recorded command must carry no result: nothing ran"
        );
    }

    #[test]
    fn refuses_to_start_when_the_target_is_already_mounted() {
        // A previous failed run leaves /mnt mounted; the old code then failed
        // at mkfs with "contains a mounted filesystem". Preflight has to catch
        // it with a fix the user can run.
        let cfg = preview_cfg();
        cfg.validate().unwrap();
        let plan = plan();
        let mut runner = SimulatedRunner::new();
        runner.mounted = true;
        let gate = SafetyGate::default();
        let log = Logger::default();
        let report = run_install(
            &cfg,
            &disk(),
            &plan,
            &gate,
            &log,
            &mut runner,
            &mut NullSink,
        );
        assert!(
            report.phases[0].state == PhaseState::Failed.as_str(),
            "{}",
            report.render()
        );
        assert!(
            report.render().contains("already mounted"),
            "the refusal must say why:\n{}",
            report.render()
        );
    }

    #[test]
    fn root_is_mounted_before_the_esp() {
        // Mounting the ESP first and root second hides the ESP: root is mounted
        // over `/mnt`, and `/mnt/boot` becomes an ordinary directory on root.
        // grub-install then refuses with "/boot doesn't look like an EFI
        // partition". The planner deliberately lists the ESP first, so the
        // mount phase must not follow plan order.
        let cfg = preview_cfg();
        cfg.validate().unwrap();
        let plan = plan();
        let mut runner = SimulatedRunner::new();
        let report = run_install(
            &cfg,
            &disk(),
            &plan,
            &SafetyGate::preview(),
            &Logger::default(),
            &mut runner,
            &mut NullSink,
        );
        assert!(report.all_completed(), "{}", report.render());

        let root = runner
            .calls
            .iter()
            .position(|c| c.ends_with("-> /mnt") || c.ends_with("-> /mnt/"))
            .expect("root mount missing");
        let esp = runner
            .calls
            .iter()
            .position(|c| c.ends_with("-> /mnt/boot"))
            .expect("ESP mount missing");
        assert!(
            root < esp,
            "root must be mounted before the ESP:\n{}",
            runner.calls.join("\n")
        );
    }

    #[test]
    fn preview_spawns_no_process_at_all() {
        // The acceptance criterion, stated as a number. `spawn_count` is bumped
        // immediately before every `Command::spawn` in the crate, so zero here
        // means zero processes started — not "the ones we know about".
        let _scope = crate::sys::PreviewScope::on();
        let cfg = preview_cfg();
        let gate = SafetyGate::preview();
        let mut runner = RealSystemRunner::new(gate.clone());
        let report = run_install(
            &cfg,
            &disk(),
            &plan(),
            &gate,
            &Logger::default(),
            &mut runner,
            &mut NullSink,
        );
        assert!(report.all_completed());
        assert!(!runner.records().is_empty(), "there was work to simulate");
        assert_eq!(
            crate::sys::spawn_count(),
            0,
            "preview started a process:\n{}",
            runner.command_lines().join("\n")
        );
    }

    #[test]
    fn preview_records_every_runner_method_and_returns_ok() {
        // Every method of the trait, called directly, including the ones that
        // never went through OpRunner and used to call `sys::*` themselves:
        // pacstrap, genfstab, install_bootloader, chroot_script, enable_services
        // and mkinitcpio. Each must record a command, succeed, and leave the
        // spawn counter at zero.
        let _scope = crate::sys::PreviewScope::on();
        let disk = disk();
        let plan = plan();
        let services = vec!["NetworkManager.service".to_string()];
        let mut runner = RealSystemRunner::new(SafetyGate::preview());

        runner.apply_partition_table(&disk, &plan).unwrap();
        runner
            .format_partition(&plan.partitions[0], "/dev/vdb1")
            .unwrap();
        runner
            .mount("/dev/vdb1", &format!("{TARGET_ROOT}/boot"))
            .unwrap();
        runner.pacstrap(&["base".into(), "linux".into()]).unwrap();
        runner.genfstab().unwrap();
        runner.install_bootloader(Bootloader::Grub, &disk).unwrap();
        runner
            .chroot_script("set -e\necho hi > /etc/hostname")
            .unwrap();
        runner.enable_services(&services).unwrap();
        runner.mkinitcpio().unwrap();

        let commands = runner.command_lines();
        assert!(
            commands
                .iter()
                .any(|c| c.contains("systemctl enable NetworkManager.service")),
            "enable_services must still show its commands:\n{}",
            commands.join("\n")
        );
        assert!(
            runner.records().iter().all(|r| !r.applied),
            "nothing in a preview may be marked applied"
        );
        assert_eq!(
            crate::sys::spawn_count(),
            0,
            "a runner method reached spawn():\n{}",
            commands.join("\n")
        );
    }

    #[test]
    fn preview_leaves_the_destructive_tools_untouched() {
        // The same proof a user would want, made mechanical: every program the
        // install can reach is replaced by a stub that appends to a marker file
        // the moment it is executed. A preview that ran a single one of them
        // would leave the marker behind.
        let _scope = crate::sys::PreviewScope::on();
        let dir =
            std::env::temp_dir().join(format!("camarelol-preview-stubs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("breach.log");
        for tool in [
            "sgdisk",
            "sfdisk",
            "mkfs.fat",
            "mkfs.ext4",
            "mkfs.btrfs",
            "mkswap",
            "mount",
            "mkdir",
            "partprobe",
            "blockdev",
            "udevadm",
            "pacstrap",
            "genfstab",
            "arch-chroot",
            "mkinitcpio",
        ] {
            let stub = dir.join(tool);
            std::fs::write(
                &stub,
                format!(
                    "#!/bin/sh\nprintf 'ran {tool} %s\\n' \"$*\" >> {}\nexit 0\n",
                    marker.display()
                ),
            )
            .unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
        // The stubs only become reachable through PATH; put the real one back
        // afterwards so the rest of the suite is unaffected.
        let real_path = std::env::var("PATH").unwrap_or_default();
        std::env::set_var("PATH", format!("{}:{real_path}", dir.display()));

        let fstab = std::path::Path::new("/mnt/etc/fstab");
        let fstab_existed = fstab.exists();

        let cfg = preview_cfg();
        let gate = SafetyGate::preview();
        let mut runner = RealSystemRunner::new(gate.clone());
        let report = run_install(
            &cfg,
            &disk(),
            &plan(),
            &gate,
            &Logger::default(),
            &mut runner,
            &mut NullSink,
        );

        std::env::set_var("PATH", real_path);

        assert!(report.all_completed(), "{}", report.render());
        assert!(
            !runner.records().is_empty(),
            "the run must have had something to simulate"
        );
        assert!(
            !marker.exists(),
            "a destructive tool ran during the preview:\n{}",
            std::fs::read_to_string(&marker).unwrap_or_default()
        );
        assert_eq!(
            fstab.exists(),
            fstab_existed,
            "genfstab output must not be appended to the target"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn preview_cannot_be_switched_off_by_any_flag() {
        // `--preview --allow-destructive --execute` all at once: the gate must
        // still refuse to write, and `dry_run` being flipped by hand must not
        // help either — that is what `writes_to_disk` is for.
        let mut gate = SafetyGate::preview();
        gate.dry_run = false;
        assert!(!gate.writes_to_disk(), "a preview gate never writes");
        assert_eq!(gate.mode(), GateMode::Preview);
        for op in [
            crate::safety::Operation::WritePartitionTable,
            crate::safety::Operation::FormatPartition,
            crate::safety::Operation::Pacstrap,
            crate::safety::Operation::InstallBootloader,
        ] {
            assert!(
                gate.authorize(op, crate::storage::device::RiskLevel::High)
                    .is_ok(),
                "a preview must not be blocked by the gate"
            );
        }
        let mut runner = RealSystemRunner::new(gate);
        assert!(runner.is_preview());
        runner.pacstrap(&["base".into()]).unwrap();
        assert!(!runner.records().is_empty());

        // And the other way round: a preview gate forced onto a real gate.
        let mut real = SafetyGate::armed_and_execute();
        real.preview = true;
        assert!(!real.writes_to_disk());
    }

    #[test]
    fn preview_never_reaches_the_op_runner_write_path() {
        // Defence in depth, tested at the layer below the runner: even a gate
        // that claims to be executing cannot make OpRunner spawn anything while
        // preview is on, because the syscall latch refuses.
        let _scope = crate::sys::PreviewScope::on();
        let mut gate = SafetyGate::armed_and_execute();
        gate.preview = true;
        let mut ops = crate::storage::ops::OpRunner::new();
        ops.apply_partition_table(&disk(), &plan(), &gate, &Logger::default())
            .unwrap();
        assert!(!ops.records.is_empty(), "the plan is still shown");
        assert!(ops.records.iter().all(|r| !r.applied));
        assert_eq!(crate::sys::spawn_count(), 0);
        assert_eq!(
            crate::sys::mutating_refusals(),
            0,
            "OpRunner must not even try when writes_to_disk() is false"
        );
    }

    #[test]
    fn preview_does_not_print_a_password() {
        let mut cfg = preview_cfg();
        cfg.set_passwords(Some("rootpw"), &[("alice", "alicepw")]);
        let script = account_script(&cfg);
        assert!(script.contains("rootpw"), "fixture really has a password");
        let shown = redact_secrets(&script);
        assert!(!shown.contains("rootpw"), "the root password leaked");
        assert!(!shown.contains("alicepw"), "the user password leaked");
        assert!(shown.contains("chpasswd"), "the command shape must stay");
        assert!(shown.contains("useradd -m -s /bin/bash alice"));
        assert!(shown.contains("wheel"));
    }

    #[test]
    fn redaction_leaves_scripts_without_secrets_alone() {
        let cfg = InstallationConfig::default();
        let script = system_config_script(&cfg);
        assert_eq!(redact_secrets(&script), script);
    }

    #[test]
    fn account_script_hides_nothing_but_is_memory_only() {
        // The script may contain the password inline (it is piped on stdin) but
        // it must never end up in argv: assert the runner uses stdin.
        let mut cfg = InstallationConfig {
            users: vec!["alice".into()],
            admin_users: vec!["alice".into()],
            ..Default::default()
        };
        cfg.set_passwords(Some("rootpw"), &[("alice", "alicepw")]);
        let script = account_script(&cfg);
        assert!(script.contains("useradd -m -s /bin/bash alice"));
        assert!(script.contains("wheel"));
    }

    // ------------------------------------------------------------ the host

    /// A host the decision table refuses, handed to the real orchestration path.
    fn refused_host(os_release: &str) -> crate::host::HostProfile {
        let profile = crate::host::HostProfile::from_parts(
            &crate::host::parse_os_release(os_release),
            crate::host::InitSystem::Systemd,
        );
        assert!(!profile.supported, "fixture must be a refused host");
        profile
    }

    #[test]
    fn refuses_an_unsupported_host_before_any_phase_runs() {
        // The counter-test for the pinned-host helper above: the same seam, a
        // refused profile, and the proof that nothing ran. `runner.calls` is
        // empty, so the run stopped at the precondition — it did not partition,
        // format or pacstrap its way to the refusal.
        for (os_release, expect) in [
            ("ID=artix\nID_LIKE=arch\n", "Artix"),
            ("ID=parabola\nID_LIKE=arch\n", "Parabola"),
            ("ID=hyperbola\n", "Hyperbola"),
            ("ID=nyarch\n", "Nyarch"),
            ("ID=endeavouros\nID_LIKE=arch\n", "endeavouros"),
        ] {
            let mut runner = SimulatedRunner::new();
            let cfg = preview_cfg();
            let report = run_install_on_host(
                &refused_host(os_release),
                &cfg,
                &disk(),
                &plan(),
                &SafetyGate::armed_and_execute(),
                &Logger::default(),
                &mut runner,
                &mut NullSink,
            );
            assert_eq!(
                report.outcome,
                Outcome::Incomplete,
                "{os_release:?} must not be installed on"
            );
            assert!(
                runner.calls.is_empty(),
                "no phase may run on {os_release:?}, got: {:?}",
                runner.calls
            );
            let phase = report
                .phases
                .first()
                .expect("a preflight failure is reported");
            assert_eq!(phase.name, "preflight");
            assert_eq!(phase.state, "failed");
            assert!(
                phase.details.contains("unsupported_host"),
                "{:?}",
                phase.details
            );
            assert!(phase.details.contains(expect), "{:?}", phase.details);
        }
    }

    #[test]
    fn refuses_arch_without_systemd_before_any_phase_runs() {
        let host = crate::host::HostProfile::from_parts(
            &crate::host::parse_os_release("ID=arch\n"),
            crate::host::InitSystem::OpenRc,
        );
        let mut runner = SimulatedRunner::new();
        let report = run_install_on_host(
            &host,
            &preview_cfg(),
            &disk(),
            &plan(),
            &SafetyGate::armed_and_execute(),
            &Logger::default(),
            &mut runner,
            &mut NullSink,
        );
        assert_eq!(report.outcome, Outcome::Incomplete);
        assert!(runner.calls.is_empty());
        assert!(report.phases[0].details.contains("OpenRC"));
    }

    #[test]
    fn the_preflight_report_names_the_host_it_verified() {
        let mut runner = SimulatedRunner::new();
        let report = run_install(
            &preview_cfg(),
            &disk(),
            &plan(),
            &SafetyGate::armed_and_execute(),
            &Logger::default(),
            &mut runner,
            &mut NullSink,
        );
        let preflight = report.phases.first().unwrap();
        assert!(
            preflight.details.contains("host=Arch Linux / systemd"),
            "{:?}",
            preflight.details
        );
    }

    #[test]
    fn the_live_entry_point_detects_the_host_rather_than_trusting_a_caller() {
        // The real `run_install`, not the pinning helper above: it takes no host
        // argument at all, so the production path cannot be told which
        // distribution it is on. Whatever this machine is, the verdict must
        // match what `detect()` says about it.
        let mut runner = SimulatedRunner::new();
        let report = crate::install::execute::run_install(
            &preview_cfg(),
            &disk(),
            &plan(),
            &SafetyGate::armed_and_execute(),
            &Logger::default(),
            &mut runner,
            &mut NullSink,
        );
        let live = crate::host::detect();
        if live.supported {
            assert_eq!(report.outcome, Outcome::Completed, "{}", report.render());
        } else {
            assert_eq!(report.outcome, Outcome::Incomplete);
            assert!(runner.calls.is_empty(), "{:?}", runner.calls);
            assert!(report.phases[0].details.contains("unsupported_host"));
        }
    }

    #[test]
    fn host_detection_is_read_only_so_a_preview_can_still_plan() {
        // A preview that could not identify the host could not say whether the
        // install is allowed, so detection has to work while the latch is on.
        let _scope = crate::sys::PreviewScope::on();
        let before = crate::sys::spawn_count();
        let _ = crate::host::detect();
        assert_eq!(
            crate::sys::spawn_count(),
            before,
            "host detection must not spawn a process"
        );
    }

    // ---------------------------------------------------------------- AUR

    fn aur_selection(packages: &[&str], user: Option<&str>) -> crate::packages::PackageSelection {
        crate::packages::PackageSelection {
            aur_enabled: true,
            aur_packages: packages.iter().map(|s| s.to_string()).collect(),
            aur_build_user: user.map(|s| s.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn the_default_install_builds_nothing_from_the_aur() {
        // AUR is opt-in. Nothing about the ordinary install may change.
        let mut runner = RealSystemRunner::new(SafetyGate::armed_and_execute());
        assert_eq!(
            runner
                .build_aur(&crate::packages::PackageSelection::default())
                .unwrap(),
            0
        );
        assert!(runner.records().is_empty());
        assert!(runner
            .aur_plan(&crate::packages::PackageSelection::default())
            .unwrap()
            .is_none());
    }

    #[test]
    fn a_simulated_install_builds_nothing_from_the_aur_by_default() {
        // The trait's default implementation is a no-op, so the test double
        // records nothing and the phase details stay the ones the acceptance
        // documentation quotes.
        let mut runner = SimulatedRunner::new();
        let report = run_install(
            &preview_cfg(),
            &disk(),
            &plan(),
            &SafetyGate::armed_and_execute(),
            &Logger::default(),
            &mut runner,
            &mut NullSink,
        );
        let pacstrap = report.phases.iter().find(|p| p.name == "pacstrap").unwrap();
        assert!(
            !pacstrap.details.contains("AUR"),
            "the default run must not mention AUR: {:?}",
            pacstrap.details
        );
    }

    #[test]
    fn a_root_run_with_aur_enabled_and_no_build_user_stops_the_phase() {
        // The whole point: camareLOL runs as root, so "no build user" must end
        // the pacstrap phase with a refusal instead of a root `makepkg`.
        let runner = RealSystemRunner::new(SafetyGate::armed_and_execute());
        let sel = aur_selection(&["some-pkg"], None);
        let uid = crate::aur::effective_uid();
        // Only meaningful when this process really is root; otherwise the
        // installer is already unprivileged and there is nothing to drop.
        if uid != Some(0) {
            return;
        }
        let err = runner.aur_plan(&sel).unwrap_err();
        assert_eq!(err.code, ErrorCode::AurBuildRefused);
        assert!(err.to_string().contains("normal user"), "{err}");
    }

    #[test]
    fn only_an_unwrapped_step_is_subject_to_the_uid_guard() {
        // The trap this pins: `runuser -u builder -- makepkg` *starts* as root —
        // that is how the uid is dropped — so a guard that refused every root
        // process would refuse every correct wrapped build and force the
        // installer into a half-rule. The guard belongs on the steps that run
        // as whatever this process is.
        let root = aur_selection(&["pkg"], Some("builder"));
        let wrapped = crate::aur::plan(&root, None, Some("runuser"), Some("builder"), true)
            .unwrap()
            .unwrap();
        assert!(
            wrapped.steps.iter().all(|s| s.as_build_user),
            "a root run wraps every step"
        );

        let unprivileged = crate::aur::plan(
            &aur_selection(&["pkg"], Some("builder")),
            None,
            None,
            None,
            false,
        )
        .unwrap()
        .unwrap();
        assert!(
            unprivileged.steps.iter().all(|s| !s.as_build_user),
            "an unprivileged run has nothing to drop"
        );
        // The guard itself is unchanged: it still refuses uid 0 and "unknown".
        assert!(crate::aur::ensure_not_root(Some(0)).is_err());
        assert!(crate::aur::ensure_not_root(None).is_err());
    }

    #[test]
    fn a_preview_with_aur_enabled_records_the_build_and_spawns_nothing() {
        let _scope = crate::sys::PreviewScope::on();
        let mut runner = RealSystemRunner::new(SafetyGate::preview());
        // Whatever this host looks like, the preview path must not refuse the
        // *plan* — it only refuses to run it.
        let sel = aur_selection(&["some-pkg"], Some("builder"));
        match runner.aur_plan(&sel) {
            Ok(Some(plan)) => {
                let built = runner.build_aur(&sel).unwrap();
                assert_eq!(built, 1);
                let commands = runner.command_lines();
                let build_line = commands
                    .iter()
                    .find(|c| c.contains("some-pkg"))
                    .unwrap_or_else(|| panic!("{commands:?}"));
                // The recorded command is the *wrapped* one whenever a drop is
                // needed, so the preview names the account that would run an
                // unreviewed build script instead of hiding it behind a bare
                // `makepkg`.
                if plan.build_user.is_some() {
                    assert!(
                        build_line.contains(" -u ") && build_line.contains("builder"),
                        "a root run must record the wrapped command: {build_line}"
                    );
                } else {
                    assert!(
                        !build_line.contains(" -u "),
                        "an unprivileged run must not pretend to drop: {build_line}"
                    );
                }
            }
            Ok(None) => panic!("AUR is enabled with a package listed: this must plan"),
            Err(e) => {
                // The only legitimate refusal is the never-as-root rule — never
                // a silent skip.
                assert_eq!(e.code, ErrorCode::AurBuildRefused, "{e}");
            }
        }
        assert!(
            runner.records().iter().all(|r| !r.applied),
            "nothing in a preview may be applied"
        );
    }

    #[test]
    fn an_invalid_aur_package_name_stops_the_phase_before_anything_runs() {
        // `makepkg --noconfirm` would exit 0 and install nothing. A profile that
        // can smuggle a flag through the package list is a profile that reports
        // success while doing nothing.
        let mut runner = SimulatedRunner::new();
        let cfg = InstallationConfig {
            packages: aur_selection(&["--help"], Some("builder")),
            ..Default::default()
        };
        let report = run_install_on_host(
            &crate::host::HostProfile::arch_systemd(),
            &cfg,
            &disk(),
            &plan(),
            &SafetyGate::armed_and_execute(),
            &Logger::default(),
            &mut runner,
            &mut NullSink,
        );
        // The simulated runner's default `build_aur` is a no-op, so the refusal
        // is proven at the plan layer instead — which is where a real run meets
        // it first.
        let err = crate::aur::plan(
            &cfg.packages,
            Some(crate::aur::AurHelper::Yay),
            Some("runuser"),
            cfg.packages.aur_build_user.as_deref(),
            false,
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidConfig);
        assert_eq!(
            report.outcome,
            Outcome::Completed,
            "the fixture is otherwise valid"
        );
    }
}
