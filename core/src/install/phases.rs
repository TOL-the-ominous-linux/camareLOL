//! Phase preconditions, success criteria and retry policy.

use super::Preflight;
use crate::boot::validate_boot_config;
use crate::error::{CamarelolError, ErrorCode, Result};
use crate::storage::layout::{validate_plan, PartitionPurpose};

/// Precondition check for a phase. Runs **before** the phase body.
pub fn preconditions(phase: &str, pf: &Preflight) -> Result<()> {
    match phase {
        "preflight" => {
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
    use crate::storage::layout::*;
    use crate::storage::scan::scan_devices_from_survey;

    fn test_preflight() -> Preflight {
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
            plan,
            config: cfg,
        }
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
}
