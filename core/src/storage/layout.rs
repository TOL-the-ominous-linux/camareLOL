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

    /// Absolute size in sectors, for the dual-boot write path that works in
    /// sectors end to end.
    pub fn sectors(&self, sector_size: u64) -> u64 {
        match self {
            SizeSpec::Bytes(b) => b / sector_size.max(1),
            SizeSpec::PercentOfDisk(_) | SizeSpec::RemainingSpace => 0,
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
    /// Real partition number, when the plan preserves an existing partition
    /// (dual-boot). `None` = a new partition that `apply_partition_table`
    /// addresses by position. With a number set, format/mount address the
    /// existing device node instead.
    #[serde(default)]
    pub number: Option<u32>,
    /// First sector of a NEW partition (dual-boot only): the planner picked it
    /// inside a validated free gap, and the preserve-write path passes it to
    /// sgdisk verbatim. `None` for kept partitions (their position is already
    /// on disk) and for whole-disk plans (positioning is sfdisk's job).
    #[serde(default)]
    pub start_sector: Option<u64>,
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
    /// True for a dual-boot plan: every existing partition of the disk is in
    /// `partitions` as a `Keep` (or reused-ESP) spec with no reformat, and the
    /// write path must create only the new partitions — never rewrite the
    /// table.
    #[serde(default)]
    pub keep_existing: bool,
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
            number: None,
            start_sector: None,
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
            number: None,
            start_sector: None,
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
            number: None,
            start_sector: None,
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
        number: None,
        start_sector: None,
    });
    n += 1;
    if req.separate_home {
        partitions.push(PartitionSpec {
            purpose: PartitionPurpose::Home,
            size: SizeSpec::Bytes(home_size),
            filesystem: Some(req.root_filesystem.clone()),
            mount_point: Some("/home".into()),
            label: Some("camarelol-home".into()),
            number: None,
            start_sector: None,
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
                number: None,
                start_sector: None,
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
        keep_existing: false,
    })
}

/// Build a dual-boot plan: keep every existing partition untouched and add
/// the Arch layout into the free space beside them.
///
/// Rules this planner enforces on itself, because a mistake here destroys a
/// real Windows install, not a test fixture:
///
/// - BIOS firmware and MBR tables are refused (v1 covers GPT + UEFI only).
/// - An existing ESP must be found; it is mounted at `/boot/efi` and **never**
///   reformatted. Arch kernels live on the root filesystem's `/boot`, because
///   a typical 100 MiB Windows ESP cannot hold them.
/// - `systemd-boot` is refused (its kernels belong on the ESP, which is the
///   one thing we may not grow).
/// - Existing partitions must be unmounted and free of any running-system
///   mount flag; the write path will bump into mounted partitions anyway,
///   and refusing here produces the readable error.
/// - Free space is located from the scanned `start` sectors. If the new
///   layout does not fit a single aligned gap in the existing layout, the
///   request is refused — silently shrinking anything is out of the question.
pub fn plan_dual_boot(device: &StorageDevice, req: &LayoutRequest) -> Result<PartitionPlan> {
    let refuse = |msg: String, remediation: &str| -> CamarelolError {
        CamarelolError::new(ErrorCode::InvalidPlan, msg).with_remediation(remediation)
    };

    if !req.root_filesystem.is_root_capable() {
        return Err(refuse(
            format!("{} cannot be used for /", req.root_filesystem.as_str()),
            "choose ext4, btrfs or xfs",
        ));
    }
    if !matches!(req.firmware, Firmware::Uefi) {
        return Err(refuse(
            "dual-boot only plans a UEFI + GPT disk (no BIOS boot code can be installed beside Windows safely this way)".to_string(),
            "boot the medium in UEFI mode, or erase the whole disk instead",
        ));
    }
    if req.bootloader != Bootloader::Grub {
        return Err(refuse(
            format!(
                "{} cannot share a Windows ESP; dual-boot needs GRUB",
                req.bootloader.as_str()
            ),
            "keep GRUB, or erase the whole disk to install systemd-boot on a fresh ESP",
        ));
    }
    if device.partition_table != PartitionTableType::Gpt {
        return Err(refuse(
            format!(
                "{} is not GPT (id: {})",
                device.path,
                device.identity()
            ),
            "dual-boot keeps existing partitions and therefore cannot create a new table; MBR systems are out of scope",
        ));
    }
    if device.partitions.is_empty() {
        return Err(refuse(
            format!("{} has no partitions to keep", device.path),
            "an empty disk should use the whole-disk automatic layout instead",
        ));
    }
    // A disk contributing to the live system must never be planned here: the
    // mount flag comes from the survey and is the same signal `--scan`
    // displays as a live system.
    let live_partitions: Vec<&str> = device
        .partitions
        .iter()
        .filter(|p| matches!(p.mount_state, super::device::MountState::Mounted { .. }))
        .map(|p| p.path.as_str())
        .collect();
    if !live_partitions.is_empty() {
        return Err(refuse(
            format!(
                "cannot keep partitions that are mounted right now: {}",
                live_partitions.join(", ")
            ),
            "boot from the Arch ISO and select this disk there",
        ));
    }

    // --- the ESP we may reuse -------------------------------------------------
    let esp = device
        .partitions
        .iter()
        .filter(|p| !p.is_windows_recovery() && !p.is_microsoft_reserved())
        .filter(|p| matches!(p.filesystem, Some(Filesystem::Vfat)))
        .find(|p| p.is_efi())
        .or_else(|| {
            // Windows' ESP sometimes reports a generic type when blkid runs
            // without privileges; a small vfat partition at the very start
            // labelled "SYSTEM" or "EFI" is still recognisable as such.
            device.partitions.iter().find(|p| {
                matches!(p.filesystem, Some(Filesystem::Vfat))
                    && p.size_bytes <= 1024 * 1024 * 1024
                    && p.label.as_deref().map(|l| {
                        let l = l.to_ascii_uppercase();
                        l.contains("EFI") || l.contains("SYSTEM")
                    }) == Some(true)
                    && !p.is_windows_recovery()
            })
        });
    let Some(esp) = esp else {
        return Err(refuse(
            format!("no EFI System Partition found on {}", device.path),
            "dual-boot reuses the existing ESP; create one (>= 100 MiB, FAT32, EFI type) or erase the whole disk",
        ));
    };

    // --- free space accounting -------------------------------------------------
    const ALIGN_SECTORS: u64 = 2048; // 1 MiB
    let sector_size = device
        .logical_sector_size
        .filter(|s| *s >= 512 && *s % 512 == 0)
        .unwrap_or(512) as u64;
    let total_sectors = device.size_bytes / sector_size;

    let mut extents: Vec<(u64, u64)> = device // (start, end_exclusive) sectors
        .partitions
        .iter()
        .filter(|p| p.start_sector > 0 && p.size_bytes > 0)
        .map(|p| (p.start_sector, p.start_sector + p.size_bytes / sector_size))
        .collect();
    extents.sort_unstable();
    extents.dedup();

    if extents.is_empty() {
        return Err(refuse(
            format!(
                "the survey for {} did not report partition starts (old lsblk output)",
                device.path
            ),
            "re-run with a current util-linux, or erase the whole disk",
        ));
    }

    // Needed size: everything except / goes before it in the same gap.
    let boot_size = if req.separate_boot { MIN_BOOT_BYTES } else { 0 };
    let home_size = if req.separate_home {
        (device.size_bytes / 10).max(GIB)
    } else {
        0
    };
    let swap_size = req.swap_bytes.unwrap_or(0);
    // Largest aligned window in any gap.
    let mut best: Option<(u64, u64)> = None; // (start_sector, end_sector_exclusive)
    let mut cursor = ALIGN_SECTORS;
    for (s, e) in extents
        .iter()
        .copied()
        .chain([(total_sectors, total_sectors)])
    {
        // candidate window: from the first 1 MiB boundary at/after `cursor`
        // up to the start of the next existing partition.
        let aligned = cursor.div_ceil(ALIGN_SECTORS) * ALIGN_SECTORS;
        let limit = s.min(total_sectors);
        if limit > aligned {
            let span = limit - aligned;
            let better = match best {
                Some((_, bs)) => span > bs,
                None => true,
            };
            if better {
                best = Some((aligned, limit));
            }
        }
        cursor = e.max(cursor);
    }
    let Some((gap_start, gap_end)) = best else {
        return Err(refuse(
            format!("no free space found on {}", device.path),
            "free space of at least the planned sizes is required; shrink or delete something first",
        ));
    };
    let gap_sectors = gap_end - gap_start;

    // ---- geometry: every new partition gets an explicit start sector ------
    //  Layout inside the gap (sectors):  [boot?][root][home?][swap?]
    //  Starts are 1 MiB aligned; / fills what the fixed pieces leave.
    let align_up = |x: u64| x.div_ceil(ALIGN_SECTORS) * ALIGN_SECTORS;
    let align_down = |x: u64| x / ALIGN_SECTORS * ALIGN_SECTORS;
    let to_sectors = |bytes: u64| bytes / sector_size;

    let boot_sectors = to_sectors(boot_size);
    let home_sectors = to_sectors(home_size);
    let swap_sectors = to_sectors(swap_size);

    // Reserved tail after / (aligned starts cost one extra alignment each).
    let mut root_end_limit = align_down(gap_end);
    if home_sectors > 0 {
        root_end_limit = root_end_limit.saturating_sub(align_up(home_sectors) + ALIGN_SECTORS);
    }
    if swap_sectors > 0 {
        root_end_limit = root_end_limit.saturating_sub(align_up(swap_sectors) + ALIGN_SECTORS);
    }
    let boot_start = gap_start; // the window already starts aligned
    let root_start = align_up(boot_start + boot_sectors);
    let root_sectors = root_end_limit.saturating_sub(root_start);
    if root_sectors < to_sectors(MIN_ROOT_BYTES) {
        return Err(refuse(
            format!(
                "the free space around sector {gap_start} on {} holds about {}, the plan needs more",
                device.path,
                human_size(gap_sectors * sector_size)
            ),
            "free more space (at least 8 GiB for /, plus /home and swap as configured) and try again",
        ));
    }
    let root_size_bytes = root_sectors * sector_size;

    // ---- assemble the plan -----------------------------------------------------
    let mut partitions: Vec<PartitionSpec> = Vec::new();
    let max_number = device
        .partitions
        .iter()
        .filter_map(|p| p.number())
        .max()
        .unwrap_or(0);
    let mut next_number = max_number;

    for p in &device.partitions {
        let is_esp = p.path == esp.path;
        partitions.push(PartitionSpec {
            purpose: if is_esp {
                PartitionPurpose::EfiSystem
            } else {
                PartitionPurpose::Keep
            },
            size: SizeSpec::Bytes(p.size_bytes),
            filesystem: p.filesystem.clone(),
            mount_point: if is_esp {
                Some("/boot/efi".into())
            } else {
                None
            },
            label: None,
            format: false,
            number: p.number(),
            start_sector: None,
        });
    }

    let mut new_spec = |purpose: PartitionPurpose,
                        start: u64,
                        size_bytes: u64,
                        filesystem: Option<Filesystem>,
                        mount_point: &str,
                        label: &str|
     -> PartitionSpec {
        next_number += 1;
        PartitionSpec {
            purpose,
            size: SizeSpec::Bytes(size_bytes),
            filesystem,
            mount_point: Some(mount_point.into()),
            label: Some(label.into()),
            format: true,
            number: Some(next_number),
            start_sector: Some(start),
        }
    };

    if req.separate_boot {
        partitions.push(new_spec(
            PartitionPurpose::Boot,
            boot_start,
            boot_size,
            Some(req.root_filesystem.clone()),
            "/boot",
            "camarelol-boot",
        ));
    }
    partitions.push(new_spec(
        PartitionPurpose::Root,
        root_start,
        root_size_bytes,
        Some(req.root_filesystem.clone()),
        "/",
        "camarelol-root",
    ));
    let mut after_root = root_start + root_sectors;
    if req.separate_home {
        let home_start = align_up(after_root);
        partitions.push(new_spec(
            PartitionPurpose::Home,
            home_start,
            home_size,
            Some(req.root_filesystem.clone()),
            "/home",
            "camarelol-home",
        ));
        after_root = home_start + home_sectors;
    }
    if req.swap_bytes.unwrap_or(0) > 0 {
        let swap_start = align_up(after_root);
        partitions.push(new_spec(
            PartitionPurpose::Swap,
            swap_start,
            swap_size,
            Some(Filesystem::Swap),
            "swap",
            "camarelol-swap",
        ));
        after_root = swap_start + swap_sectors;
    }
    // The geometry may never exceed the gap; sgdisk would refuse the write,
    // but catching it here turns a mystery into a planner bug report.
    if after_root > gap_end {
        return Err(refuse(
            "the planner produced a layout that overflows the free gap (bug)".to_string(),
            "report it; nothing was written",
        ));
    }

    debug_assert!(partitions
        .iter()
        .any(|p| p.purpose == PartitionPurpose::Root));

    Ok(PartitionPlan {
        disk_path: device.path.clone(),
        disk_identity: device.identity(),
        disk_risk: RiskLevel::High, // keeping Windows is the highest-stakes plan
        table_type: device.partition_table.clone(),
        partitions,
        keep_existing: true,
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
    use super::super::device::MountState;
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

    const GPT_EFI_TYPE: &str = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b";
    const GPT_MSR_TYPE: &str = "e3c9e316-0b5c-4db8-817d-f92df00215ae";

    /// A Windows-shaped GPT disk: ESP, MSR, a 40 GiB C: partition, and a
    /// 1 GiB recovery partition at the very end — with ~40 GiB free in the
    /// middle, which is what shrinking C: leaves behind.
    fn windows_disk() -> StorageDevice {
        const SECTORS: u64 = 512;
        let gb = 1024 * 1024 * 1024;
        let mk = |name: &str, n: u32, start: u64, size: u64, ty: Option<&str>, fs, tyname| {
            super::super::device::Partition {
                kernel_name: name.into(),
                path: format!("/dev/nvme0n1p{n}"),
                label: None,
                uuid: None,
                part_uuid: None,
                size_bytes: size * SECTORS,
                start_sector: start,
                filesystem: fs,
                partition_type: ty.map(|t| t.to_string()),
                partition_type_name: tyname,
                mount_state: MountState::NotMounted,
                os_hints: vec![],
                part_label: None,
            }
        };
        StorageDevice {
            kernel_name: "nvme0n1".into(),
            path: "/dev/nvme0n1".into(),
            model: None,
            vendor: None,
            serial: Some("win-disk".into()),
            wwn: None,
            size_bytes: 80 * gb,
            logical_sector_size: Some(512),
            physical_sector_size: Some(512),
            rotational: false,
            removable: false,
            transport: None,
            kind: super::super::device::DeviceKind::NvmeSsd,
            partition_table: PartitionTableType::Gpt,
            partitions: vec![
                mk(
                    "nvme0n1p1",
                    1,
                    2048,
                    204_800,
                    Some(GPT_EFI_TYPE),
                    Some(Filesystem::Vfat),
                    None,
                ),
                mk(
                    "nvme0n1p2",
                    2,
                    206_848,
                    32_768,
                    Some(GPT_MSR_TYPE),
                    None,
                    None,
                ),
                mk(
                    "nvme0n1p3",
                    3,
                    307_200,
                    83_886_080,
                    None,
                    Some(Filesystem::Ntfs),
                    None,
                ),
                mk(
                    "nvme0n1p4",
                    4,
                    165_675_008,
                    2_097_152,
                    None,
                    Some(Filesystem::Ntfs),
                    Some("Windows recovery".to_string()),
                ),
            ],
            mount_state: MountState::NotMounted,
            os_hints: vec![],
            encrypted: None,
            risk: RiskLevel::High,
        }
    }

    #[test]
    fn dual_boot_keeps_windows_and_reuses_the_esp() {
        let plan = plan_dual_boot(&windows_disk(), &req_uefi()).unwrap();
        assert!(plan.keep_existing);
        // Every existing partition is present, and none of them gets formatted.
        let windows_specs: Vec<&PartitionSpec> = plan
            .partitions
            .iter()
            .filter(|p| p.number == Some(3))
            .collect();
        assert_eq!(windows_specs.len(), 1, "C: must be kept");
        assert!(!windows_specs[0].format);
        assert_eq!(windows_specs[0].purpose, PartitionPurpose::Keep);
        // The reused ESP is a real ESP spec (the boot config checks look for
        // the purpose), mounted under /boot, never reformatted.
        let esp = plan
            .partitions
            .iter()
            .find(|p| p.purpose == PartitionPurpose::EfiSystem)
            .expect("ESP reused");
        assert!(!esp.format);
        assert_eq!(esp.number, Some(1));
        assert_eq!(esp.mount_point.as_deref(), Some("/boot/efi"));
        // The new root goes into the middle gap, gets the next number.
        let root = plan
            .partitions
            .iter()
            .find(|p| p.purpose == PartitionPurpose::Root)
            .expect("root present");
        assert_eq!(root.number, Some(5));
        assert_eq!(root.start_sector, Some(84_193_280));
        assert!(root.format);
        // ...and nothing the planner wants overlaps the kept recovery
        // partition that sits after the gap.
        let end_sector = root.start_sector.unwrap() + root.size.sectors(512) - 1;
        assert!(
            end_sector < 165_675_008,
            "root would overlap the recovery partition"
        );
    }

    #[test]
    fn dual_boot_refuses_without_esp() {
        let mut d = windows_disk();
        d.partitions.retain(|p| p.number() != Some(1));
        let err = plan_dual_boot(&d, &req_uefi()).unwrap_err();
        assert!(err.message.contains("no EFI System Partition"));
    }

    #[test]
    fn dual_boot_refuses_mbr_and_bios() {
        let mut d = windows_disk();
        d.partition_table = PartitionTableType::Mbr;
        assert!(plan_dual_boot(&d, &req_uefi()).is_err());

        let mut req = req_uefi();
        req.firmware = Firmware::Bios;
        assert!(plan_dual_boot(&windows_disk(), &req).is_err());
    }

    #[test]
    fn dual_boot_refuses_a_mounted_disk() {
        let mut d = windows_disk();
        d.partitions[2].mount_state = MountState::Mounted {
            target: "/target".into(),
            live_system: false,
        };
        let err = plan_dual_boot(&d, &req_uefi()).unwrap_err();
        assert!(err.message.contains("mounted right now"));
    }

    #[test]
    fn dual_boot_refuses_a_tiny_gap() {
        // Shrink the gap to less than the minimum root: the planner must say
        // what is missing instead of producing a plan that cannot fit.
        let mut d = windows_disk();
        d.partitions[3].start_sector = 94_960_640; // ~10 GiB of free space left
        let err = plan_dual_boot(&d, &req_uefi()).unwrap_err();
        assert!(err.message.contains("holds about"));
    }

    #[test]
    fn dual_boot_numbers_never_collide_and_starts_never_overlap() {
        for swap in [None, Some(2 * 1024 * 1024 * 1024)] {
            let mut req = req_uefi();
            req.swap_bytes = swap;
            let plan = plan_dual_boot(&windows_disk(), &req).unwrap();
            let mut nums: Vec<u32> = plan.partitions.iter().filter_map(|p| p.number).collect();
            nums.sort_unstable();
            nums.dedup();
            assert_eq!(
                nums.len(),
                plan.partitions.len(),
                "partition numbers unique"
            );
            // Every new partition starts after the last kept end, and before
            // the recovery partition.
            for spec in plan.partitions.iter().filter(|p| p.format) {
                let s = spec.start_sector.unwrap();
                // C: occupies up to sector 84_193_280 (exclusive).
                assert!(s >= 84_193_280, "starts inside the gap");
                let e = s + spec.size.sectors(512) - 1;
                assert!(e < 165_675_008, "ends before the recovery partition");
            }
        }
    }

    #[test]
    fn sgdisk_append_only_touches_new_partitions() {
        let plan = plan_dual_boot(&windows_disk(), &req_uefi()).unwrap();
        let batches = super::super::ops::sgdisk_append_args(&plan).unwrap();
        assert_eq!(batches.len(), 1, "one root partition to create");
        let block: &Vec<String> = &batches[0];
        assert!(
            block.iter().any(|a| a.starts_with("--new=5:")),
            "sgdisk must append with the explicit number: {block:?}"
        );
        assert!(
            !block.iter().any(|a| a.starts_with("--new=1:")
                || a.starts_with("--new=2:")
                || a.starts_with("--new=3:")
                || a.starts_with("--new=4:")),
            "no kept partition may be recreated: {block:?}"
        );
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
                number: None,
                start_sector: None,
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
