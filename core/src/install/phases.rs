//! Phase preconditions, success criteria and retry policy.

use super::Preflight;
use crate::boot::validate_boot_config;
use crate::error::{CamarelolError, ErrorCode, Result};
use crate::storage::layout::{validate_plan, PartitionPurpose};

/// Precondition check for a phase. Runs **before** the phase body.
pub fn preconditions(phase: &str, pf: &Preflight) -> Result<()> {
    match phase {
        "preflight" => {
            // The host first, before anything else: no point validating a plan
            // on a distribution whose package names and init layout differ from
            // the ones this installer was built against. This runs at plan
            // time, so an Artix or Hyperbola machine is turned away before a
            // single byte is written — and the error says which host it is.
            pf.host.require_supported()?;
            // The configuration itself must be valid before anything is written.
            pf.config.validate()?;
            validate_plan(&pf.plan, &pf.plan_request())?;
            validate_boot_config(&pf.plan, pf.firmware, pf.config.bootloader()?)?;
            // sanity: hardware preconditions of the desktop profile
            let profile = pf.config.desktop_profile()?;
            crate::desktop::validate_desktop_config(
                profile,
                pf.config.display_server()?,
                pf.config.display_manager.as_deref(),
            )?;
            Ok(())
        }
        "partition" => {
            if pf.plan.partitions.is_empty() {
                return Err(CamarelolError::new(
                    ErrorCode::InvalidPlan,
                    "no partitions planned",
                ));
            }
            Ok(())
        }
        "format" => {
            let root = pf
                .plan
                .partitions
                .iter()
                .find(|p| p.purpose == PartitionPurpose::Root);
            match root {
                Some(r) if r.format => Ok(()),
                Some(_) => Err(CamarelolError::new(
                    ErrorCode::InvalidPlan,
                    "root partition is marked keep-existing but has no filesystem",
                )),
                None => Err(CamarelolError::new(
                    ErrorCode::InvalidPlan,
                    "plan has no root partition",
                )),
            }
        }
        "mount" => {
            let root_mount = pf
                .plan
                .partitions
                .iter()
                .any(|p| p.mount_point.as_deref() == Some("/"));
            if !root_mount {
                return Err(CamarelolError::new(
                    ErrorCode::InvalidPlan,
                    "nothing is mounted at /",
                ));
            }
            Ok(())
        }
        "pacstrap" => Ok(()),
        "fstab" => Ok(()),
        "bootloader" => {
            // firmware-specific bootloader preconditions were validated in preflight
            validate_boot_config(&pf.plan, pf.firmware, pf.config.bootloader()?)?;
            Ok(())
        }
        "system-config" | "accounts" | "services" | "initramfs" => {
            // target must be mounted, checked by the runner
            Ok(())
        }
        "post-check" => Ok(()),
        other => Err(CamarelolError::new(
            ErrorCode::Internal,
            format!("unknown phase `{other}`"),
        )),
    }
}

/// Whether a failed phase may be re-run.
pub fn retryable(phase: &str) -> bool {
    matches!(
        phase,
        "pacstrap"
            | "fstab"
            | "bootloader"
            | "system-config"
            | "accounts"
            | "services"
            | "initramfs"
    )
}

impl Preflight {
    /// Build the layout request implied by the configuration.
    pub fn plan_request(&self) -> crate::storage::layout::LayoutRequest {
        crate::storage::layout::LayoutRequest {
            firmware: self.firmware,
            bootloader: self
                .config
                .bootloader()
                .unwrap_or(crate::boot::Bootloader::Grub),
            root_filesystem: self
                .config
                .filesystem_type()
                .unwrap_or(crate::storage::device::Filesystem::Ext4),
            separate_home: self.config.separate_home,
            separate_boot: self.config.separate_boot,
            swap_bytes: self.config.swap_bytes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boot::Bootloader;
    use crate::config::InstallationConfig;
    use crate::hardware::Firmware;
    use crate::host::{parse_os_release, Distro, HostProfile, InitSystem};
    use crate::storage::layout::*;
    use crate::storage::scan::scan_devices_from_survey;

    fn test_preflight() -> Preflight {
        test_preflight_on(HostProfile::arch_systemd())
    }

    /// Same plan, a different host. Every host test goes through here so the
    /// only variable between them really is the host.
    fn test_preflight_on(host: HostProfile) -> Preflight {
        let devices = scan_devices_from_survey(
            r#"[{"kernel_name":"vdb","path":"/dev/vdb","size_bytes":21474836480,"rotational":false,"removable":false,"kind":"Virtual","partition_table":"None","partitions":[],"mount_state":"NotMounted","os_hints":[],"encrypted":null,"risk":"Low"}]"#,
        )
        .unwrap();
        let cfg = InstallationConfig::default();
        let req = LayoutRequest {
            firmware: Firmware::Uefi,
            bootloader: Bootloader::Grub,
            root_filesystem: crate::storage::device::Filesystem::Ext4,
            separate_home: false,
            separate_boot: false,
            swap_bytes: None,
        };
        let plan = plan_auto_layout(&devices[0], &req).unwrap();
        Preflight {
            firmware: Firmware::Uefi,
            host,
            plan,
            config: cfg,
        }
    }

    /// The profile the decision table says a given `/etc/os-release` produces.
    fn host_for(os_release: &str, init: InitSystem) -> HostProfile {
        HostProfile::from_parts(&parse_os_release(os_release), init)
    }

    #[test]
    fn preflight_passes_for_a_valid_plan() {
        let pf = test_preflight();
        assert!(preconditions("preflight", &pf).is_ok());
    }

    #[test]
    fn preflight_fails_when_esp_missing() {
        let mut pf = test_preflight();
        pf.plan
            .partitions
            .retain(|p| p.purpose != PartitionPurpose::EfiSystem);
        let err = preconditions("preflight", &pf).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidBootConfig);
    }

    #[test]
    fn mount_phase_requires_root_mount() {
        let mut pf = test_preflight();
        pf.plan.partitions.clear();
        assert!(preconditions("mount", &pf).is_err());
    }

    #[test]
    fn retryable_policy() {
        assert!(retryable("pacstrap"));
        assert!(!retryable("partition"));
    }

    // ------------------------------------------------------------ the host

    #[test]
    fn preflight_refuses_a_non_arch_distribution_before_anything_is_written() {
        // Same plan, same config, same firmware as the passing test above: the
        // only difference is the host, so the refusal cannot be a side effect
        // of the layout.
        for (os_release, expected) in [
            ("ID=artix\nID_LIKE=arch\n", Distro::Artix),
            ("ID=nyarch\nID_LIKE=arch\n", Distro::Nyarch),
            ("ID=hyperbola\nID_LIKE=parabola arch\n", Distro::Hyperbola),
            ("ID=parabola\nID_LIKE=arch\n", Distro::Parabola),
            ("ID=endeavouros\nID_LIKE=arch\n", Distro::Unknown),
            ("ID=manjaro\nID_LIKE=arch\n", Distro::Unknown),
            ("", Distro::Unknown),
        ] {
            let host = host_for(os_release, InitSystem::Systemd);
            assert_eq!(host.distro, expected, "for {os_release:?}");
            let err = preconditions("preflight", &test_preflight_on(host)).unwrap_err();
            assert_eq!(
                err.code,
                ErrorCode::UnsupportedHost,
                "{os_release:?} must be refused at plan time"
            );
            assert!(
                err.remediation.is_some(),
                "the refusal must tell the user what to do: {err}"
            );
        }
    }

    #[test]
    fn preflight_refuses_arch_without_systemd() {
        for init in [
            InitSystem::OpenRc,
            InitSystem::Runit,
            InitSystem::Dinit,
            InitSystem::Unknown,
        ] {
            let host = host_for("ID=arch\n", init);
            let err = preconditions("preflight", &test_preflight_on(host)).unwrap_err();
            assert_eq!(err.code, ErrorCode::UnsupportedHost, "{}", init.as_str());
        }
    }

    #[test]
    fn the_refusal_names_the_host_it_found() {
        let host = host_for(
            "NAME=\"Hyperbola GNU/Linux-libre\"\nID=hyperbola\n",
            InitSystem::Systemd,
        );
        let err = preconditions("preflight", &test_preflight_on(host)).unwrap_err();
        let text = err.to_string();
        assert!(text.contains("Hyperbola"), "{text}");
        assert!(
            text.contains("not** Arch"),
            "the decision belongs in the message: {text}"
        );
        assert_eq!(
            err.context.as_deref(),
            Some("ID=\"hyperbola\" detected from os-release"),
            "{text}"
        );
    }

    #[test]
    fn arch_with_systemd_is_still_allowed_after_the_host_check() {
        // The guard above must not have broken the one host that works.
        let host = host_for("ID=arch\nNAME=\"Arch Linux\"\n", InitSystem::Systemd);
        assert!(host.supported);
        assert!(preconditions("preflight", &test_preflight_on(host)).is_ok());
    }

    #[test]
    fn the_host_check_comes_before_the_plan_is_trusted() {
        // Both a bad host *and* a broken plan: the host is what the user has to
        // fix first, and reporting the plan would be answering the wrong
        // question.
        let mut pf = test_preflight_on(host_for("ID=artix\n", InitSystem::Systemd));
        pf.plan.partitions.clear();
        let err = preconditions("preflight", &pf).unwrap_err();
        assert_eq!(err.code, ErrorCode::UnsupportedHost);
    }
}
