//! Boot mode and bootloader configuration.

use crate::error::{CamarelolError, ErrorCode, Result};
use crate::hardware::Firmware;
use crate::storage::layout::{PartitionPurpose, PartitionPlan};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Bootloader {
    SystemdBoot,
    Grub,
}

impl Bootloader {
    pub fn as_str(&self) -> &'static str {
        match self {
            Bootloader::SystemdBoot => "systemd-boot",
            Bootloader::Grub => "GRUB",
        }
    }

    pub fn supports_uefi(&self) -> bool {
        true
    }

    pub fn supports_bios(&self) -> bool {
        matches!(self, Bootloader::Grub)
    }

    /// Packages needed *inside the target system* to install this bootloader.
    ///
    /// They must exist before `arch-chroot /mnt grub-install` can run, so the
    /// install orchestrator adds them to the pacstrap list.
    pub fn target_packages(&self) -> &'static [&'static str] {
        match self {
            Bootloader::Grub => &["grub", "efibootmgr"],
            Bootloader::SystemdBoot => &["systemd", "efibootmgr"],
        }
    }

    /// Packages needed in the target system.
    pub fn packages(&self) -> &'static [&'static str] {
        match self {
            Bootloader::SystemdBoot => &["systemd", "systemd-boot"],
            Bootloader::Grub => &["grub"],
        }
    }
}

/// Validate that the chosen bootloader really can be installed for this plan.
pub fn validate_boot_config(plan: &PartitionPlan, firmware: Firmware, bootloader: Bootloader) -> Result<()> {
    let has_root = plan
        .partitions
        .iter()
        .any(|p| p.purpose == PartitionPurpose::Root && p.mount_point.as_deref() == Some("/"));
    if !has_root {
        return Err(CamarelolError::new(
            ErrorCode::InvalidBootConfig,
            "the plan has no partition mounted at /",
        ));
    }

    match firmware {
        Firmware::Uefi => {
            let esp = plan
                .partitions
                .iter()
                .any(|p| p.purpose == PartitionPurpose::EfiSystem);
            if !esp {
                return Err(CamarelolError::new(
                    ErrorCode::InvalidBootConfig,
                    "UEFI boot requires an EFI System Partition",
                ));
            }
        }
        Firmware::Bios => {
            if !bootloader.supports_bios() {
                return Err(CamarelolError::new(
                    ErrorCode::InvalidBootConfig,
                    format!("{} does not support BIOS systems", bootloader.as_str()),
                ));
            }
        }
        Firmware::Unknown => {
            return Err(CamarelolError::new(
                ErrorCode::InvalidBootConfig,
                "firmware mode is unknown",
            ));
        }
    }

    // Every partition needs a mount point (except bios boot / swap / keep).
    for p in &plan.partitions {
        match p.purpose {
            PartitionPurpose::BiosBoot | PartitionPurpose::Keep => continue,
            _ => {
                if p.mount_point.is_none() {
                    return Err(CamarelolError::new(
                        ErrorCode::InvalidPlan,
                        format!("{} has no mount point", p.purpose.as_str()),
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Summary shown in the Review screen.
pub fn boot_summary(firmware: Firmware, bootloader: Bootloader, esp_path: Option<&str>) -> String {
    let esp = esp_path.unwrap_or("(new ESP created by camareLOL)");
    format!(
        "{} firmware | {} | ESP: {}",
        firmware.as_str(),
        bootloader.as_str(),
        esp
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::device::*;
    use crate::storage::layout::*;

    fn plan_with(purpose: PartitionPurpose, mount: &str) -> PartitionPlan {
        PartitionPlan {
            disk_path: "/dev/vda".into(),
            disk_identity: "test".into(),
            disk_risk: RiskLevel::Low,
            table_type: PartitionTableType::Gpt,
            partitions: vec![
                PartitionSpec {
                    purpose: PartitionPurpose::EfiSystem,
                    size: SizeSpec::Bytes(1024 * 1024 * 1024),
                    filesystem: Some(Filesystem::Vfat),
                    mount_point: Some("/boot".into()),
                    label: Some("ESP".into()),
                    format: true,
                },
                PartitionSpec {
                    purpose,
                    size: SizeSpec::RemainingSpace,
                    filesystem: Some(Filesystem::Ext4),
                    mount_point: Some(mount.into()),
                    label: Some("root".into()),
                    format: true,
                },
            ],
        }
    }

    #[test]
    fn uefi_requires_esp() {
        let ok = plan_with(PartitionPurpose::Root, "/");
        assert!(validate_boot_config(&ok, Firmware::Uefi, Bootloader::SystemdBoot).is_ok());

        let mut broken = plan_with(PartitionPurpose::Root, "/");
        broken.partitions.remove(0);
        let err = validate_boot_config(&broken, Firmware::Uefi, Bootloader::SystemdBoot).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidBootConfig);
    }

    #[test]
    fn systemd_boot_rejected_for_bios() {
        let plan = plan_with(PartitionPurpose::Root, "/");
        // remove ESP because BIOS must not have one
        let mut p = plan.clone();
        p.partitions.remove(0);
        let err = validate_boot_config(&p, Firmware::Bios, Bootloader::SystemdBoot).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidBootConfig);
    }

    #[test]
    fn root_mount_required() {
        let plan = plan_with(PartitionPurpose::Home, "/home");
        assert!(validate_boot_config(&plan, Firmware::Uefi, Bootloader::Grub).is_err());
    }
}
