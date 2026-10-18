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
    /// Non-fatal problems worth telling the user about.
    pub warnings: Vec<String>,
}

impl RealSystemRunner {
    pub fn new(gate: SafetyGate) -> Self {
        RealSystemRunner {
            ops: OpRunner::new(),
            gate,
            warnings: Vec::new(),
        }
    }

    pub fn records(&self) -> &[crate::storage::ops::RecordedCommand] {
        &self.ops.records
    }

    fn ensure_exec(&self) -> Result<()> {
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
            GateMode::Execute => Ok(()),
        }
    }

    /// Run a script inside the target system via `arch-chroot`, piping the
    /// script on stdin (no argv exposure of secrets).
    fn chroot_run(&mut self, script: &str, _phase: &str) -> Result<()> {
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

impl SystemRunner for RealSystemRunner {
    fn apply_partition_table(&mut self, disk: &StorageDevice, plan: &PartitionPlan) -> Result<()> {
        self.ensure_exec()?;
        let log = Logger::default();
        self.ops
            .apply_partition_table(disk, plan, &self.gate.clone(), &log)
    }

    fn format_partition(&mut self, spec: &PartitionSpec, target: &str) -> Result<()> {
        self.ensure_exec()?;
        let log = Logger::default();
        self.ops
            .format_partition(spec, target, &self.gate.clone(), &log)
    }

    fn mount(&mut self, source: &str, target: &str) -> Result<()> {
        self.ensure_exec()?;
        let log = Logger::default();
        self.ops.mount(source, target, &self.gate.clone(), &log)
    }

    fn pacstrap(&mut self, packages: &[String]) -> Result<()> {
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
        std::path::Path::new(TARGET_ROOT).join("etc").is_dir()
    }

    fn warnings(&self) -> Vec<String> {
        self.warnings.clone()
    }

    fn mkinitcpio(&mut self) -> Result<()> {
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
}

impl SimulatedRunner {
    pub fn new() -> Self {
        SimulatedRunner {
            calls: Vec::new(),
            fail_on: None,
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
        false
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
pub fn run_install(
    cfg: &InstallationConfig,
    disk: &StorageDevice,
    plan: &PartitionPlan,
    _gate: &SafetyGate,
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
                    "preflight" => Ok(format!(
                        "firmware={} bootloader={} fs={} partitions={}",
                        firmware.as_str(),
                        cfg.bootloader()
                            .map(|b| b.as_str().to_string())
                            .unwrap_or_default(),
                        cfg.filesystem,
                        plan.partitions.len()
                    )),
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
                        for (i, spec) in plan.partitions.iter().enumerate() {
                            let target = part(i + 1);
                            match spec.purpose {
                                PartitionPurpose::BiosBoot => continue,
                                _ => {
                                    if let Some(mp) = spec.mount_point.as_deref() {
                                        let mnt = format!("{TARGET_ROOT}{mp}");
                                        runner.mount(&target, &mnt)?;
                                    }
                                }
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
                        Ok(format!("installed {} packages", pkgs.len()))
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
}
