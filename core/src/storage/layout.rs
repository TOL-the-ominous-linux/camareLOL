//! Partition planning.
//!
//! camareLOL never writes a filesystem onto a bare disk as a shortcut: a plan is
//! always a full, validated partition layout (see [`PartitionPlan`]) that is
//! re-checked against the live device immediately before anything is written.

use super::device::{human_size, Filesystem, PartitionTableType, RiskLevel, StorageDevice};
use crate::boot::Bootloader;
use crate::error::{CamarelolError, ErrorCode, Result};
use crate::hardware::Firmware;
use serde::{Deserialize, Serialize};

/// What a partition is for.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PartitionPurpose {
    EfiSystem,
    BiosBoot,
    Boot,
    Root,
    Home,
    Swap,
    Data,
    Keep,
}

impl PartitionPurpose {
    pub fn as_str(&self) -> &'static str {
        match self {
            PartitionPurpose::EfiSystem => "EFI System Partition",
            PartitionPurpose::BiosBoot => "BIOS boot partition",
            PartitionPurpose::Boot => "/boot",
            PartitionPurpose::Root => "/",
            PartitionPurpose::Home => "/home",
            PartitionPurpose::Swap => "swap",
            PartitionPurpose::Data => "data",
            PartitionPurpose::Keep => "keep existing",
        }
    }
}

/// How big a planned partition should be.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SizeSpec {
    Bytes(u64),
    PercentOfDisk(u32),
    RemainingSpace,
}

impl SizeSpec {
    pub fn display(&self) -> String {
        match self {
            SizeSpec::Bytes(b) => human_size(*b),
            SizeSpec::PercentOfDisk(p) => format!("{p}%"),
            SizeSpec::RemainingSpace => "remaining".to_string(),
        }
    }
}

/// One planned partition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionSpec {
    pub purpose: PartitionPurpose,
    pub size: SizeSpec,
    pub filesystem: Option<Filesystem>,
    pub mount_point: Option<String>,
    pub label: Option<String>,
    /// Reformat even if the target already holds a filesystem (always requires
    /// an explicit high-risk confirmation before it is allowed).
    pub format: bool,
}

impl PartitionSpec {
    pub fn describe(&self) -> String {
        let fs = self.filesystem.as_ref().map(|f| f.as_str()).unwrap_or("—");
        let mp = self.mount_point.clone().unwrap_or_else(|| "—".into());
        let fmt = if self.format { "format" } else { "keep" };
        format!(
            "{:<24} {:<8} {:<10} {:<12} {}",
            self.purpose.as_str(),
            fs,
            self.size.display(),
            mp,
            fmt
        )
    }
}

/// A complete, validated layout for one disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionPlan {
    pub disk_path: String,
    pub disk_identity: String,
    pub disk_risk: RiskLevel,
    pub table_type: PartitionTableType,
    pub partitions: Vec<PartitionSpec>,
}

impl PartitionPlan {
    /// Human readable diagram used by the Review screen.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "Disk:      {} ({})\n",
            self.disk_path, self.disk_identity
        ));
        out.push_str(&format!("Table:     {}\n", self.table_type));
        out.push_str(&format!("Risk:      {}\n", self.disk_risk.as_str()));
        out.push('\n');
        out.push_str(&format!(
            "{:<24} {:<8} {:<10} {:<12} {}\n",
            "PURPOSE", "FS", "SIZE", "MOUNTPOINT", "ACTION"
        ));
        out.push_str(&"-".repeat(72));
        out.push('\n');
        for p in &self.partitions {
            out.push_str(&p.describe());
            out.push('\n');
        }
        out
    }

    /// Every operation in this plan that destroys data.
    pub fn destructive_operations(&self) -> Vec<String> {
        let mut ops = Vec::new();
        if self.table_type != PartitionTableType::None {
            ops.push(format!(
                "write {} partition table on {}",
                self.table_type, self.disk_path
            ));
        }
        for p in &self.partitions {
            if p.format {
                ops.push(format!(
                    "format {} as {}",
                    p.purpose.as_str(),
                    p.filesystem.as_ref().map(|f| f.as_str()).unwrap_or("?")
                ));
            }
        }
        ops
    }
}

/// What the user asked for.
#[derive(Debug, Clone)]
pub struct LayoutRequest {
    pub firmware: Firmware,
    pub bootloader: Bootloader,
    pub root_filesystem: Filesystem,
    pub separate_home: bool,
    pub separate_boot: bool,
    pub swap_bytes: Option<u64>,
}

impl Default for LayoutRequest {
    fn default() -> Self {
        LayoutRequest {
            firmware: Firmware::Uefi,
            bootloader: Bootloader::Grub,
            root_filesystem: Filesystem::Ext4,
            separate_home: false,
            separate_boot: false,
            swap_bytes: None,
        }
    }
}

const GIB: u64 = 1024 * 1024 * 1024;
const MIB: u64 = 1024 * 1024;

/// Minimum sensible sizes, in bytes.
pub const MIN_ESP_BYTES: u64 = 512 * MIB;
pub const MIN_ROOT_BYTES: u64 = 8 * GIB;
pub const MIN_BIOS_BOOT_BYTES: u64 = MIB;
pub const MIN_BOOT_BYTES: u64 = GIB;

/// Build the automatic plan for a disk.
///
/// `fs` must be a root-capable filesystem; the bootloader decides whether a
/// BIOS boot partition or an ESP is required for the detected firmware.
pub fn plan_auto_layout(device: &StorageDevice, req: &LayoutRequest) -> Result<PartitionPlan> {
    if !req.root_filesystem.is_root_capable() {
        return Err(CamarelolError::new(
            ErrorCode::InvalidPlan,
            format!("{} cannot be used for /", req.root_filesystem.as_str()),
        )
        .with_remediation("choose ext4, btrfs or xfs"));
    }

    let total = device.size_bytes;
    let efi = matches!(req.firmware, Firmware::Uefi);
    let bios_boot = matches!(req.firmware, Firmware::Bios)
        && matches!(req.bootloader, Bootloader::Grub)
        && device.partition_table != PartitionTableType::Mbr;

    let esp_size = if efi { GIB } else { 0 };
    let bios_boot_size = if bios_boot { MIN_BIOS_BOOT_BYTES } else { 0 };
    let boot_size = if req.separate_boot { MIN_BOOT_BYTES } else { 0 };
    let home_size = if req.separate_home {
        (total / 10).max(GIB)
    } else {
        0
    };
    let swap_size = req.swap_bytes.unwrap_or(0);

    let fixed = esp_size + bios_boot_size + boot_size + home_size + swap_size;
    if fixed >= total {
        return Err(CamarelolError::new(
            ErrorCode::InvalidPlan,
            format!("disk {} is too small for the requested layout", device.path),
        )
        .with_remediation("remove /home /boot/swap or pick a larger disk"));
    }
    let root_size = total - fixed;
    if root_size < MIN_ROOT_BYTES {
        return Err(CamarelolError::new(
            ErrorCode::InvalidPlan,
            format!(
                "only {} left for / on {} (need at least {})",
                human_size(root_size),
                device.path,
                human_size(MIN_ROOT_BYTES)
            ),
        ));
    }

    let mut partitions = Vec::new();
    let mut n = 1;
    let table = if efi || bios_boot || device.partition_table == PartitionTableType::Gpt {
        PartitionTableType::Gpt
    } else {
        PartitionTableType::Mbr
    };

    if efi {
        partitions.push(PartitionSpec {
            purpose: PartitionPurpose::EfiSystem,
            size: SizeSpec::Bytes(esp_size),
            filesystem: Some(Filesystem::Vfat),
            // With a separate /boot partition the ESP must move to /boot/efi;
            // both claiming /boot was a real collision — the second mount
            // hid the first and grub-install rejected the ESP.
            mount_point: Some(
                if req.separate_boot {
                    "/boot/efi"
                } else {
                    "/boot"
                }
                .into(),
            ),
            // FAT labels are limited to 11 characters. `mkfs.fat` refuses
            // anything longer, and the old "CAMARELOL-EFI" (13) stopped every
            // UEFI install at the format phase.
            label: Some("CAMAROL-EFI".into()),
            format: true,
        });
        n += 1;
    }
    if bios_boot {
        partitions.push(PartitionSpec {
            purpose: PartitionPurpose::BiosBoot,
            size: SizeSpec::Bytes(bios_boot_size),
            filesystem: None,
            mount_point: None,
            label: None,
            format: false,
        });
        n += 1;
    }
    if req.separate_boot {
        partitions.push(PartitionSpec {
            purpose: PartitionPurpose::Boot,
            size: SizeSpec::Bytes(boot_size),
            filesystem: Some(req.root_filesystem.clone()),
            mount_point: Some("/boot".into()),
            label: Some("camarelol-boot".into()),
            format: true,
        });
        n += 1;
    }
    partitions.push(PartitionSpec {
        purpose: PartitionPurpose::Root,
        size: SizeSpec::Bytes(root_size),
        filesystem: Some(req.root_filesystem.clone()),
        mount_point: Some("/".into()),
        label: Some("camarelol-root".into()),
        format: true,
    });
    n += 1;
    if req.separate_home {
        partitions.push(PartitionSpec {
            purpose: PartitionPurpose::Home,
            size: SizeSpec::Bytes(home_size),
            filesystem: Some(req.root_filesystem.clone()),
            mount_point: Some("/home".into()),
            label: Some("camarelol-home".into()),
            format: true,
        });
        n += 1;
    }
    if let Some(sz) = req.swap_bytes {
        if sz > 0 {
            partitions.push(PartitionSpec {
                purpose: PartitionPurpose::Swap,
                size: SizeSpec::Bytes(sz),
                filesystem: Some(Filesystem::Swap),
                mount_point: Some("swap".into()),
                label: Some("camarelol-swap".into()),
                format: true,
            });
            n += 1;
        }
    }
    debug_assert!(n > 1);

    Ok(PartitionPlan {
        disk_path: device.path.clone(),
        disk_identity: device.identity(),
        disk_risk: device.assess_risk(),
        table_type: table,
        partitions,
    })
}

/// Validate a plan against firmware/bootloader/filesystem rules.
pub fn validate_plan(plan: &PartitionPlan, req: &LayoutRequest) -> Result<()> {
    if plan.partitions.is_empty() {
        return Err(CamarelolError::new(
            ErrorCode::InvalidPlan,
            "plan has no partitions",
        ));
    }

    let root_count = plan
        .partitions
        .iter()
        .filter(|p| p.purpose == PartitionPurpose::Root)
        .count();
    if root_count != 1 {
        return Err(CamarelolError::new(
            ErrorCode::InvalidPlan,
            "plan must contain exactly one root partition",
        ));
    }

    // Two partitions claiming the same mountpoint cannot both be mounted;
    // the second hides (or fails over) the first. The plan above moved the
    // ESP to /boot/efi for exactly this reason, and a plan that arrives from
    // a saved configuration gets caught here instead.
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for p in &plan.partitions {
        if let Some(mp) = p.mount_point.as_deref() {
            // "swap" is an fstab marker, not a directory under the target.
            if p.purpose == PartitionPurpose::Swap {
                continue;
            }
            if !seen.insert(mp) {
                return Err(CamarelolError::new(
                    ErrorCode::InvalidPlan,
                    format!("two partitions claim the same mount point {mp}"),
                )
                .with_remediation("give them distinct mountpoints, or drop one of them"));
            }
        }
    }

    // systemd-boot looks for kernels on the ESP (or an XBOOTLDR partition,
    // which the planner does not create). A separate ext4 /boot would get the
    // kernels and stay invisible to the bootloader.
    if req.bootloader == Bootloader::SystemdBoot
        && plan
            .partitions
            .iter()
            .any(|p| p.purpose == PartitionPurpose::Boot)
    {
        return Err(CamarelolError::new(
            ErrorCode::InvalidBootConfig,
            "systemd-boot cannot use a separate ext4 /boot",
        )
        .with_remediation(
            "drop the separate /boot (kernels then live on the ESP) or install GRUB",
        ));
    }

    match req.firmware {
        Firmware::Uefi => {
            let esp = plan
                .partitions
                .iter()
                .any(|p| p.purpose == PartitionPurpose::EfiSystem);
            if !esp {
                return Err(CamarelolError::new(
                    ErrorCode::InvalidBootConfig,
                    "UEFI installation requires an EFI System Partition",
                )
                .with_remediation("add an ESP (FAT32, >= 512 MiB) to the layout"));
            }
        }
        Firmware::Bios => {
            let has_esp = plan
                .partitions
                .iter()
                .any(|p| p.purpose == PartitionPurpose::EfiSystem);
            if has_esp {
                return Err(CamarelolError::new(
                    ErrorCode::InvalidBootConfig,
                    "an ESP is useless on a BIOS system",
                )
                .with_remediation("remove the ESP or boot the medium in UEFI mode"));
            }
            if req.bootloader == Bootloader::Grub
                && plan.table_type == PartitionTableType::Gpt
                && !plan
                    .partitions
                    .iter()
                    .any(|p| p.purpose == PartitionPurpose::BiosBoot)
            {
                return Err(CamarelolError::new(
                    ErrorCode::InvalidBootConfig,
                    "GPT + BIOS + GRUB needs a BIOS boot partition",
                ));
            }
        }
        Firmware::Unknown => {
            return Err(CamarelolError::new(
                ErrorCode::InvalidBootConfig,
                "cannot detect the firmware mode",
            )
            .with_remediation(
                "boot the Arch medium in UEFI mode (recommended) or make sure the VM/BIOS reports its mode",
            ));
        }
    }

    // Bootloader must be supported by the table type.
    if let (Bootloader::SystemdBoot, PartitionTableType::Mbr) = (req.bootloader, &plan.table_type) {
        return Err(CamarelolError::new(
            ErrorCode::InvalidBootConfig,
            "systemd-boot cannot boot from an MBR system",
        ));
    }

    // XFS + GRUB without a dedicated /boot: GRUB support for XFS is limited.
    if req.root_filesystem == Filesystem::Xfs
        && req.bootloader == Bootloader::Grub
        && !req.separate_boot
        && !plan
            .partitions
            .iter()
            .any(|p| p.purpose == PartitionPurpose::Boot)
    {
        return Err(CamarelolError::new(
            ErrorCode::InvalidPlan,
            "GRUB with XFS on / requires a separate /boot partition",
        )
        .with_remediation("enable a separate /boot or use ext4/btrfs"));
    }

    // XFS + systemd-boot: kernels live on the ESP, so it is technically fine.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_disk() -> StorageDevice {
        StorageDevice {
            kernel_name: "vda".into(),
            path: "/dev/vda".into(),
            model: None,
            vendor: None,
            serial: Some("vm-disk".into()),
            wwn: None,
            size_bytes: 20 * GIB,
            logical_sector_size: Some(512),
            physical_sector_size: Some(512),
            rotational: false,
            removable: false,
            transport: None,
            kind: super::super::device::DeviceKind::Virtual,
            partition_table: PartitionTableType::None,
            partitions: vec![],
            mount_state: super::super::device::MountState::NotMounted,
            os_hints: vec![],
            encrypted: None,
            risk: RiskLevel::Low,
        }
    }

    fn req_uefi() -> LayoutRequest {
        LayoutRequest::default()
    }

    #[test]
    fn uefi_empty_disk_gets_esp_and_root() {
        let plan = plan_auto_layout(&empty_disk(), &req_uefi()).unwrap();
        assert_eq!(plan.table_type, PartitionTableType::Gpt);
        let esp = plan
            .partitions
            .iter()
            .find(|p| p.purpose == PartitionPurpose::EfiSystem)
            .expect("ESP present");
        assert_eq!(esp.filesystem, Some(Filesystem::Vfat));
        assert!(plan
            .partitions
            .iter()
            .any(|p| p.purpose == PartitionPurpose::Root));
        validate_plan(&plan, &req_uefi()).unwrap();
    }

    #[test]
    fn every_partition_label_fits_its_filesystem() {
        let req = LayoutRequest {
            separate_boot: true,
            separate_home: true,
            swap_bytes: Some(GIB),
            ..Default::default()
        };
        let plan = plan_auto_layout(&empty_disk(), &req).unwrap();
        for part in &plan.partitions {
            let (Some(filesystem), Some(label)) = (&part.filesystem, &part.label) else {
                continue;
            };
            // mkfs.fat accepts at most 11 characters; ext4, btrfs and swap
            // accept 16. A longer ESP label made every UEFI install fail.
            let limit = match filesystem {
                Filesystem::Vfat => 11,
                _ => 16,
            };
            assert!(
                label.len() <= limit,
                "{filesystem:?} label {label:?} is {} characters, maximum is {limit}",
                label.len()
            );
        }
    }

    #[test]
    fn bios_grub_gpt_gets_bios_boot_partition() {
        let req = LayoutRequest {
            firmware: Firmware::Bios,
            bootloader: Bootloader::Grub,
            ..Default::default()
        };
        let plan = plan_auto_layout(&empty_disk(), &req).unwrap();
        assert!(plan
            .partitions
            .iter()
            .any(|p| p.purpose == PartitionPurpose::BiosBoot));
        validate_plan(&plan, &req).unwrap();
    }

    #[test]
    fn bios_systemd_boot_is_rejected() {
        let req = LayoutRequest {
            firmware: Firmware::Bios,
            bootloader: Bootloader::SystemdBoot,
            ..Default::default()
        };
        let plan = plan_auto_layout(&empty_disk(), &req).unwrap();
        let err = validate_plan(&plan, &req).unwrap_err();
        // systemd-boot cannot be installed on an MBR/BIOS system.
        assert_eq!(err.code, ErrorCode::InvalidBootConfig);
    }

    #[test]
    fn xfs_grub_requires_separate_boot() {
        let req = LayoutRequest {
            root_filesystem: Filesystem::Xfs,
            firmware: Firmware::Uefi,
            bootloader: Bootloader::Grub,
            ..Default::default()
        };
        let plan = plan_auto_layout(&empty_disk(), &req).unwrap();
        assert!(validate_plan(&plan, &req).is_err());

        let req2 = LayoutRequest {
            separate_boot: true,
            ..req.clone()
        };
        let plan2 = plan_auto_layout(&empty_disk(), &req2).unwrap();
        validate_plan(&plan2, &req2).unwrap();
    }

    #[test]
    fn separate_boot_moves_the_esp_to_boot_efi() {
        // Old planner mounted the ESP at /boot even with a separate /boot
        // partition — the second mount hid the first and grub-install
        // rejected the ESP. The two mountpoints must stay distinct.
        let req = LayoutRequest {
            separate_boot: true,
            ..Default::default()
        };
        let plan = plan_auto_layout(&empty_disk(), &req).unwrap();
        let esp = plan
            .partitions
            .iter()
            .find(|p| p.purpose == PartitionPurpose::EfiSystem)
            .expect("ESP present");
        assert_eq!(esp.mount_point.as_deref(), Some("/boot/efi"));
        let boot = plan
            .partitions
            .iter()
            .find(|p| p.purpose == PartitionPurpose::Boot)
            .expect("boot present");
        assert_eq!(boot.mount_point.as_deref(), Some("/boot"));
        validate_plan(&plan, &req).unwrap();
        assert_ne!(esp.mount_point, boot.mount_point);
    }

    #[test]
    fn validate_rejects_two_partitions_on_one_mountpoint() {
        // A plan shape that 0.1.17 could produce from a saved configuration:
        // ESP and separate /boot both said /boot.
        let mut plan = plan_auto_layout(&empty_disk(), &req_uefi()).unwrap();
        plan.partitions.insert(
            1,
            PartitionSpec {
                purpose: PartitionPurpose::Boot,
                size: SizeSpec::Bytes(MIN_BOOT_BYTES),
                filesystem: Some(Filesystem::Ext4),
                mount_point: Some("/boot".into()),
                label: Some("camarelol-boot".into()),
                format: true,
            },
        );
        let err = validate_plan(&plan, &req_uefi()).unwrap_err();
        assert!(
            err.message.contains("same mount point"),
            "the refusal must name the collision: {}",
            err.message
        );
    }

    #[test]
    fn validate_rejects_systemd_boot_with_separate_boot() {
        let req = LayoutRequest {
            bootloader: Bootloader::SystemdBoot,
            separate_boot: true,
            ..Default::default()
        };
        let plan = plan_auto_layout(&empty_disk(), &req).unwrap();
        let err = validate_plan(&plan, &req).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidBootConfig);
        assert!(
            err.message.to_lowercase().contains("systemd-boot"),
            "the refusal must name the bootloader: {}",
            err.message
        );
    }

    #[test]
    fn tiny_disk_is_rejected_with_explanation() {
        let mut tiny = empty_disk();
        tiny.size_bytes = GIB;
        let err = plan_auto_layout(&tiny, &req_uefi()).unwrap_err();
        assert!(err.remediation.is_some() || !err.message.is_empty());
    }

    #[test]
    fn plan_lists_destructive_operations() {
        let plan = plan_auto_layout(&empty_disk(), &req_uefi()).unwrap();
        let ops = plan.destructive_operations();
        assert!(ops.len() >= 2);
        assert!(plan.render().contains("Disk:"));
    }
}
