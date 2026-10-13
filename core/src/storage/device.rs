//! Storage device model.
//!
//! The model deliberately does **not** rely on `/dev/sda`-style names as
//! identity: identity is the tuple (`wwn` | `serial` | kernel_name) so a plan
//! created on one boot can be re-validated against a possibly renamed device
//! on the next boot (see [`StorageDevice::identity_strength`]).

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeviceKind {
    NvmeSsd,
    SataSsd,
    SataHdd,
    Usb,
    Mmc,
    Virtual,
    Other,
}

impl DeviceKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeviceKind::NvmeSsd => "NVMe SSD",
            DeviceKind::SataSsd => "SATA SSD",
            DeviceKind::SataHdd => "SATA HDD",
            DeviceKind::Usb => "USB Drive",
            DeviceKind::Mmc => "MMC",
            DeviceKind::Virtual => "Virtual Disk",
            DeviceKind::Other => "Disk",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PartitionTableType {
    Gpt,
    Mbr,
    None,
    Unknown,
}

impl fmt::Display for PartitionTableType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            PartitionTableType::Gpt => "GPT",
            PartitionTableType::Mbr => "MBR",
            PartitionTableType::None => "no partition table",
            PartitionTableType::Unknown => "unknown table",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Filesystem {
    Vfat,
    Ext2,
    Ext4,
    Btrfs,
    Xfs,
    Ntfs,
    Exfat,
    Swap,
    Luks,
    LvmPv,
    LinuxRaidMember,
    ZfsMember,
    Other(String),
}

impl Filesystem {
    pub fn from_str_loose(s: &str) -> Self {
        let l = s.to_ascii_lowercase();
        match l.as_str() {
            "vfat" | "fat32" | "fat16" | "msdos" => Filesystem::Vfat,
            "ext2" => Filesystem::Ext2,
            "ext4" => Filesystem::Ext4,
            "btrfs" => Filesystem::Btrfs,
            "xfs" => Filesystem::Xfs,
            "ntfs" | "ntfs3" => Filesystem::Ntfs,
            "exfat" => Filesystem::Exfat,
            "swap" => Filesystem::Swap,
            "crypto_luks" | "luks" => Filesystem::Luks,
            "lvm2 pv" | "lvm2_member" | "lvm" => Filesystem::LvmPv,
            "linux_raid_member" => Filesystem::LinuxRaidMember,
            "zfs_member" => Filesystem::ZfsMember,
            other => Filesystem::Other(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Filesystem::Vfat => "vfat",
            Filesystem::Ext2 => "ext2",
            Filesystem::Ext4 => "ext4",
            Filesystem::Btrfs => "btrfs",
            Filesystem::Xfs => "xfs",
            Filesystem::Ntfs => "ntfs",
            Filesystem::Exfat => "exfat",
            Filesystem::Swap => "swap",
            Filesystem::Luks => "crypto_LUKS",
            Filesystem::LvmPv => "LVM2_member",
            Filesystem::LinuxRaidMember => "linux_raid_member",
            Filesystem::ZfsMember => "zfs_member",
            Filesystem::Other(s) => s.as_str(),
        }
    }

    /// Filesystem usable as a Linux root filesystem by camareLOL.
    pub fn is_root_capable(&self) -> bool {
        matches!(self, Filesystem::Ext4 | Filesystem::Btrfs | Filesystem::Xfs)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MountState {
    NotMounted,
    Mounted { target: String, live_system: bool },
    Busy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OsSignature {
    Windows,
    EfiSystemPartition,
    WindowsRecovery,
    MicrosoftReserved,
    LinuxSwap,
    LinuxRoot,
    LinuxHome,
    Other(String),
}

impl OsSignature {
    pub fn as_str(&self) -> &str {
        match self {
            OsSignature::Windows => "Windows",
            OsSignature::EfiSystemPartition => "EFI System Partition",
            OsSignature::WindowsRecovery => "Windows recovery",
            OsSignature::MicrosoftReserved => "Microsoft Reserved",
            OsSignature::LinuxSwap => "swap",
            OsSignature::LinuxRoot => "Linux root",
            OsSignature::LinuxHome => "Linux home",
            OsSignature::Other(s) => s.as_str(),
        }
    }
}

/// Risk level of touching a device, shown in the UI before any confirmation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RiskLevel {
    /// Nothing valuable found; device looks unused.
    Low,
    /// Device holds data the user may want to keep.
    Medium,
    /// Another operating system or the live system is using this device.
    High,
}

impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskLevel::Low => "low risk",
            RiskLevel::Medium => "medium risk",
            RiskLevel::High => "high risk",
        }
    }
}

/// A partition (child of a disk).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Partition {
    pub kernel_name: String,
    pub path: String,
    pub label: Option<String>,
    pub uuid: Option<String>,
    pub part_uuid: Option<String>,
    pub size_bytes: u64,
    pub filesystem: Option<Filesystem>,
    pub partition_type: Option<String>,
    pub partition_type_name: Option<String>,
    pub mount_state: MountState,
    pub os_hints: Vec<OsSignature>,
    /// blkid PARTLABEL / PARTTYPE human string, if known.
    pub part_label: Option<String>,
}

impl Partition {
    pub fn is_efi(&self) -> bool {
        self.partition_type
            .as_deref()
            .map(|t| t.eq_ignore_ascii_case("c12a7328-f81f-11d2-ba4b-00a0c93ec93b"))
            .unwrap_or(false)
            || self
                .part_label
                .as_deref()
                .map(|l| l.to_ascii_lowercase().contains("efi"))
                .unwrap_or(false)
    }

    pub fn is_windows_recovery(&self) -> bool {
        self.partition_type_name
            .as_deref()
            .map(|t| t.to_ascii_lowercase().contains("recovery"))
            .unwrap_or(false)
            || (self.filesystem == Some(Filesystem::Ntfs)
                && self
                    .part_label
                    .as_deref()
                    .map(|l| l.to_ascii_lowercase().contains("recovery"))
                    .unwrap_or(false))
    }

    pub fn is_microsoft_reserved(&self) -> bool {
        self.partition_type
            .as_deref()
            .map(|t| t.eq_ignore_ascii_case("e3c9e316-0b5c-4db8-817d-f92df00215ae"))
            .unwrap_or(false)
    }

    pub fn has_filesystem(&self) -> bool {
        self.filesystem.is_some()
    }
}

/// How strongly we can identify a device.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum IdentityStrength {
    /// Only the kernel name is known (weakest; e.g. USB without serial).
    KernelNameOnly,
    /// Serial number available (stable across reboots).
    Serial,
    /// WWN available (strongest).
    Wwn,
}

impl IdentityStrength {
    pub fn as_str(&self) -> &'static str {
        match self {
            IdentityStrength::KernelNameOnly => "kernel name only",
            IdentityStrength::Serial => "serial",
            IdentityStrength::Wwn => "WWN",
        }
    }
}

/// A physical disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageDevice {
    pub kernel_name: String,
    pub path: String,
    pub model: Option<String>,
    pub vendor: Option<String>,
    pub serial: Option<String>,
    pub wwn: Option<String>,
    pub size_bytes: u64,
    pub logical_sector_size: Option<u32>,
    pub physical_sector_size: Option<u32>,
    pub rotational: bool,
    pub removable: bool,
    pub transport: Option<String>,
    pub kind: DeviceKind,
    pub partition_table: PartitionTableType,
    pub partitions: Vec<Partition>,
    pub mount_state: MountState,
    pub os_hints: Vec<OsSignature>,
    pub encrypted: Option<bool>,
    /// Risk assessment computed by [`StorageDevice::assess_risk`].
    pub risk: RiskLevel,
}

impl StorageDevice {
    /// Identity string used to re-validate a plan right before writing.
    pub fn identity(&self) -> String {
        self.wwn
            .clone()
            .or_else(|| self.serial.clone())
            .unwrap_or_else(|| self.kernel_name.clone())
    }

    pub fn identity_strength(&self) -> IdentityStrength {
        if self.wwn.is_some() {
            IdentityStrength::Wwn
        } else if self.serial.is_some() {
            IdentityStrength::Serial
        } else {
            IdentityStrength::KernelNameOnly
        }
    }

    pub fn capacity_display(&self) -> String {
        human_size(self.size_bytes)
    }

    pub fn is_empty(&self) -> bool {
        self.partition_table == PartitionTableType::None && self.partitions.is_empty()
    }

    pub fn has_windows(&self) -> bool {
        self.os_hints.contains(&OsSignature::Windows)
            || self.partitions.iter().any(|p| {
                p.filesystem == Some(Filesystem::Ntfs)
                    || p.os_hints.contains(&OsSignature::Windows)
                    || p.os_hints.contains(&OsSignature::WindowsRecovery)
            })
    }

    pub fn used_by_live_system(&self) -> bool {
        matches!(
            &self.mount_state,
            MountState::Mounted {
                live_system: true,
                ..
            }
        ) || self.partitions.iter().any(|p| {
            matches!(
                &p.mount_state,
                MountState::Mounted {
                    live_system: true,
                    ..
                }
            )
        })
    }

    pub fn efi_partitions(&self) -> Vec<&Partition> {
        self.partitions.iter().filter(|p| p.is_efi()).collect()
    }

    /// Compute the risk level of modifying this device.
    /// Pure function of the scanned state so it can be unit tested.
    pub fn assess_risk(&self) -> RiskLevel {
        if self.used_by_live_system() {
            return RiskLevel::High;
        }
        if self.has_windows() {
            return RiskLevel::High;
        }
        if self.partitions.iter().any(|p| p.has_filesystem()) {
            return RiskLevel::Medium;
        }
        if self.partition_table == PartitionTableType::None && self.partitions.is_empty() {
            return RiskLevel::Low;
        }
        RiskLevel::Medium
    }

    /// One-line summary used by list UIs.
    pub fn summary_line(&self) -> String {
        let count = self.partitions.len();
        let table = self.partition_table.to_string();
        let parts = if count == 0 {
            "no partitions".to_string()
        } else {
            format!("{count} partition{}", if count == 1 { "" } else { "s" })
        };
        let os = if self.has_windows() {
            " | Windows detected"
        } else {
            ""
        };
        format!(
            "{} — {} | {} | {}{}",
            self.kind.as_str(),
            self.capacity_display(),
            table,
            parts,
            os
        )
    }
}

/// Format a byte count as e.g. `512 GiB`.
pub fn human_size(bytes: u64) -> String {
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const KIB: f64 = 1024.0;
    let b = bytes as f64;
    if b >= GIB {
        format!("{:.0} GiB", b / GIB)
    } else if b >= MIB {
        format!("{:.0} MiB", b / MIB)
    } else if b >= KIB {
        format!("{:.0} KiB", b / KIB)
    } else {
        format!("{bytes} B")
    }
}

/// Parse a human size like `512GiB`, `20 GiB`, `512M`, `1024` (bytes).
pub fn parse_human_size(input: &str) -> Option<u64> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    let upper = s.to_ascii_uppercase().replace(' ', "");
    let (num, mult) = if let Some(v) = upper.strip_suffix("KIB") {
        (v, 1024u64)
    } else if let Some(v) = upper.strip_suffix("MIB") {
        (v, 1024 * 1024)
    } else if let Some(v) = upper.strip_suffix("GIB") {
        (v, 1024 * 1024 * 1024)
    } else if let Some(v) = upper.strip_suffix("TIB") {
        (v, 1024u64 * 1024 * 1024 * 1024)
    } else if let Some(v) = upper.strip_suffix('K').or_else(|| upper.strip_suffix("KB")) {
        (v, 1000)
    } else if let Some(v) = upper.strip_suffix('M').or_else(|| upper.strip_suffix("MB")) {
        (v, 1000 * 1000)
    } else if let Some(v) = upper.strip_suffix('G').or_else(|| upper.strip_suffix("GB")) {
        (v, 1000 * 1000 * 1000)
    } else if let Some(v) = upper.strip_suffix('T').or_else(|| upper.strip_suffix("TB")) {
        (v, 1000u64 * 1000 * 1000 * 1000)
    } else {
        (upper.as_str(), 1)
    };
    num.parse::<u64>().ok().map(|n| n.saturating_mul(mult))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn disk() -> StorageDevice {
        StorageDevice {
            kernel_name: "sdb".into(),
            path: "/dev/sdb".into(),
            model: Some("USB Flash".into()),
            vendor: None,
            serial: Some("123".into()),
            wwn: None,
            size_bytes: 32 * 1024 * 1024 * 1024,
            logical_sector_size: Some(512),
            physical_sector_size: Some(512),
            rotational: false,
            removable: true,
            transport: Some("usb".into()),
            kind: DeviceKind::Usb,
            partition_table: PartitionTableType::Gpt,
            partitions: vec![],
            mount_state: MountState::NotMounted,
            os_hints: vec![],
            encrypted: None,
            risk: RiskLevel::Low,
        }
    }

    #[test]
    fn empty_disk_is_low_risk() {
        let mut d = disk();
        d.partition_table = PartitionTableType::None;
        assert!(d.is_empty());
        assert_eq!(d.assess_risk(), RiskLevel::Low);
    }

    #[test]
    fn windows_disk_is_high_risk() {
        let mut d = disk();
        d.partitions = vec![Partition {
            kernel_name: "sdb1".into(),
            path: "/dev/sdb1".into(),
            label: None,
            uuid: None,
            part_uuid: None,
            size_bytes: 100,
            filesystem: Some(Filesystem::Ntfs),
            partition_type: None,
            partition_type_name: Some("Basic data partition".into()),
            mount_state: MountState::NotMounted,
            os_hints: vec![],
            part_label: None,
        }];
        assert!(d.has_windows());
        assert_eq!(d.assess_risk(), RiskLevel::High);
    }

    #[test]
    fn human_size_round_trip() {
        assert_eq!(human_size(1024 * 1024 * 1024), "1 GiB");
        assert_eq!(parse_human_size("1 GiB"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_human_size("512M"), Some(512_000_000));
        assert_eq!(parse_human_size("garbage"), None);
    }

    #[test]
    fn identity_prefers_wwn_then_serial_then_name() {
        let mut d = disk();
        assert_eq!(d.identity(), "123");
        d.wwn = Some("wwn-1".into());
        assert_eq!(d.identity(), "wwn-1");
        assert_eq!(d.identity_strength(), IdentityStrength::Wwn);
    }

    #[test]
    fn efi_partition_detected_by_type_or_label() {
        let mut d = disk();
        d.partitions = vec![
            Partition {
                kernel_name: "sdb1".into(),
                path: "/dev/sdb1".into(),
                label: None,
                uuid: None,
                part_uuid: None,
                size_bytes: 100,
                filesystem: Some(Filesystem::Vfat),
                partition_type: Some("c12a7328-f81f-11d2-ba4b-00a0c93ec93b".into()),
                partition_type_name: None,
                mount_state: MountState::NotMounted,
                os_hints: vec![],
                part_label: None,
            },
            Partition {
                kernel_name: "sdb2".into(),
                path: "/dev/sdb".into(),
                label: None,
                uuid: None,
                part_uuid: None,
                size_bytes: 100,
                filesystem: Some(Filesystem::Vfat),
                partition_type: None,
                partition_type_name: None,
                mount_state: MountState::NotMounted,
                os_hints: vec![],
                part_label: Some("EFI System Partition".into()),
            },
        ];
        assert_eq!(d.efi_partitions().len(), 2);
    }
}
