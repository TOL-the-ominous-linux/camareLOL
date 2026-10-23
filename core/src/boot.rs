//! Boot mode and bootloader configuration.

use crate::error::{CamarelolError, ErrorCode, Result};
use crate::hardware::Firmware;
use crate::storage::layout::{PartitionPlan, PartitionPurpose};
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
            // `bootctl` ships inside the `systemd` package. There is no package
            // called `systemd-boot` in the Arch repositories — asking for one
            // made pacstrap fail with "target not found: systemd-boot" on every
            // systemd-boot install, and no unit test could see it because the
            // list is only ever resolved by the package manager.
            Bootloader::SystemdBoot => &["systemd"],
            Bootloader::Grub => &["grub"],
        }
    }
}

// --------------------------------------------------------------------------
// EFI boot entry cleanup
//
// Firmware accumulates boot entries: earlier install attempts on the same
// ESP, USB installers that were booted once, abandoned loader names. After a
// reinstall the firmware may point at a `Boot0005` whose `EFI/…` file no
// longer exists anywhere and refuse to fall through to the entry that does —
// the machine hangs at "no bootable device" although the freshly installed
// loader is right there. These helpers exist so the bootloader phase can
// observe `efibootmgr -v` and delete exactly the entries that point into
// OUR ESP at files that are not there.
//
// Conservative by construction: only `HD(...)/File(...)` entries whose
// partition GUID matches OUR ESP can ever be deleted, and only when the
// loader file is missing from that ESP. `FvFile`, `CdRom`, `Usb` devices and
// any other OS's ESP are never touched.

/// One `Boot####` line of `efibootmgr -v`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EfiBootEntry {
    pub num: u16,
    pub label: String,
    /// Loader path inside the ESP, converted to Unix form
    /// (`\EFI\foo\bar.efi` → `EFI/foo/bar.efi`), lowercase (ESP is FAT32,
    /// case-insensitive). `None` when the entry has no `File(…)` component
    /// at all (`FvFile`, `Usb`, firmware built-ins).
    pub loader: Option<String>,
    /// Partition GUID inside the following `HD(<n>,gpt,<guid>,…)` descriptor.
    pub hd_guid: Option<String>,
    /// Partition number of the `HD(…)` descriptor, when GPT.
    pub hd_part: Option<u32>,
}

/// Parse the whole `efibootmgr -v` listing.
///
/// Real firmware lines look like
/// `Boot0005* camareLOL→HD(2,GPT,d26c…,0x800,0x2000)/File(\EFI\camareLOL\grubx64.efi)`
/// or `Boot0000* UiApp→FvFile(462CAA21-…)`. Parsing survives both: the
/// separator and quoting of labels varies by vendor.
pub fn parse_efi_entries(listing: &str) -> Vec<EfiBootEntry> {
    let mut out = Vec::new();
    for line in listing.lines() {
        let line = line.trim();
        if line.len() <= 8 || !line.starts_with("Boot") {
            continue;
        }
        // After "Boot" come exactly four hex digits, then '*' or ' '.
        let Some(hex) = line.get(4..8) else {
            continue;
        };
        let Ok(num) = u16::from_str_radix(hex, 16) else {
            continue;
        };
        // Drop the mechanism marker '*', then find where the device path
        // begins: its first device-class token. Everything before is the
        // vendor-chosen label.
        let rest = line[8..].trim_start_matches('*').trim();
        let path_start = [
            "HD(",
            "CdRom(",
            "FvFile(",
            "PcieRoot(",
            "Usb(",
            "SATA(",
            "VenHv(",
        ]
        .iter()
        .filter_map(|t| rest.find(t))
        .min();
        let (label, path) = match path_start {
            Some(i) => (rest[..i].trim(), &rest[i..]),
            None => (rest, ""),
        };
        // Loader: /File(\EFI\…) — Windows-style path, backslash separators,
        // always introduced by "/File(" (matching plain "File(" would also
        // catch the middle of "FvFile(…)" — a firmware built-in).
        let loader = path.find("/File(").and_then(|i| {
            let inner = &path[i + 6..];
            let close = inner.find(')')?;
            let raw: &str = &inner[..close];
            if raw.is_empty() {
                return None;
            }
            Some(
                raw.trim_start_matches('\\')
                    .replace('\\', "/")
                    .to_lowercase(),
            )
        });
        // HD descriptor: HD(<part>,<type>,<guid>,…)
        let (hd_part, hd_guid) = path
            .find("HD(")
            .and_then(|i| {
                let inner = &path[i + 3..];
                let close = inner.find(')')?;
                Some(inner[..close].to_string())
            })
            .and_then(|inner| {
                let parts: Vec<&str> = inner.split(',').collect();
                if parts.len() < 3 {
                    return None;
                }
                let part = parts[0].trim().parse::<u32>().ok();
                let guid = parts[2].trim().to_lowercase();
                (!guid.is_empty()).then_some((part, Some(guid)))
            })
            .unwrap_or((None, None));
        out.push(EfiBootEntry {
            num,
            label: label.to_string(),
            loader,
            hd_guid,
            hd_part,
        });
    }
    out.sort_by_key(|e| e.num);
    out
}

/// Entries that should be deleted: they point into the ESP whose partition
/// GUID is `esp_guid` and the loader file is absent from it.
///
/// `loader_exists` receives the ESP-relative path already converted (`EFI/…`).
/// Anything without a loader or without an HD descriptor is never returned —
/// over-deletion is the failure mode this function refuses. When `esp_guid`
/// is `None` (caller could not learn our ESP's GUID) the GUID guard
/// collapses to "has any HD descriptor on some disk", which is the honest
/// limit of what the listing alone can say; the callers in the installer
/// always pass a GUID.
pub fn stale_boot_entries<'a>(
    entries: &'a [EfiBootEntry],
    esp_guid: Option<&str>,
    loader_exists: &dyn Fn(&str) -> bool,
) -> Vec<&'a EfiBootEntry> {
    let ours = esp_guid.map(|g| g.to_lowercase());
    entries
        .iter()
        .filter(|e| {
            let Some(loader) = e.loader.clone() else {
                return false;
            };
            let Some(guid) = e.hd_guid.clone() else {
                return false;
            };
            match &ours {
                Some(hay) => *hay == guid,
                None => e.hd_part.is_some(),
            }
            .then(|| loader_exists(&loader))
            .map(|exists| !exists)
            .unwrap_or(false)
        })
        .collect()
}

/// Validate that the chosen bootloader really can be installed for this plan.
pub fn validate_boot_config(
    plan: &PartitionPlan,
    firmware: Firmware,
    bootloader: Bootloader,
) -> Result<()> {
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

    /// Sample listing shaped after a VBox UEFI whose disk was reinstalled:
    /// firmware built-in, DVD, a live USB, one leftover attempt and the
    /// current install — plus a Windows entry from a dual-boot.
    const LISTING: &str = "\
BootCurrent: 0002
Timeout: 1 seconds
BootOrder: 0002,0005,0001
Boot0000* UiApp\tFvFile(462CAA21-7614-4536-8FAE-CD509748061D)
Boot0001* UEFI VBOX CD-ROM VB2-0171d3f6-3e2c37f8\tPciRoot(0x0)/Pci(0x1,0x1)/Ata(0,0,0)/File(\\EFI\\BOOT\\BOOTX64.EFI)
Boot0002* camareLOL\tHD(1,GPT,d26c4b8a-1234-4187-abcd-00e3a37bd45f,0x800,0x30000)/File(\\EFI\\camareLOL\\grubx64.efi)
Boot0003* UEFI: Misc USB\tPciRoot(0x0)/Pci(0xd,0x0)/Usb(1,0)/File(\\EFI\\BOOT\\BOOTX64.EFI)
Boot0005* camareLOL-old-test\tHD(1,GPT,d26c4b8a-1234-4187-abcd-00e3a37bd45f,0x800,0x30000)/File(\\EFI\\archtest\\grubx64.efi)
Boot0006* Windows Boot Manager\tHD(1,GPT,d26c4b8a-1234-4187-abcd-00e3a37bd45f,0x800,0x30000)/File(\\EFI\\Microsoft\\Boot\\bootmgfw.efi)
Boot0007* EndeavourOS\tHD(1,GPT,000dead2-9999-4187-abcd-00e3a37bd45f,0x800,0x30000)/File(\\EFI\\endeavouros\\grubx64.efi)";

    #[test]
    fn parses_entries_and_excludes_non_entry_lines() {
        let e = parse_efi_entries(LISTING);
        // Header lines (BootCurrent/Timeout/BootOrder) never become entries.
        assert_eq!(e.len(), 7, "{e:?}");
        assert_eq!(e[2].num, 2);
        assert_eq!(e[2].label, "camareLOL");
        assert_eq!(e[2].loader.as_deref(), Some("efi/camarelol/grubx64.efi"));
        assert_eq!(
            e[2].hd_guid.as_deref(),
            Some("d26c4b8a-1234-4187-abcd-00e3a37bd45f")
        );
        assert_eq!(e[2].hd_part, Some(1));
        // FvFile — firmware built-in: no File() at all.
        assert_eq!(e[0].loader, None);
        // CdRom/Usb entries carry File() but no HD descriptor.
        assert!(e[1].hd_guid.is_none(), "{e:?}");
        assert!(e[3].hd_guid.is_none(), "{e:?}");
        assert_eq!(e[4].loader.as_deref(), Some("efi/archtest/grubx64.efi"));
    }

    #[test]
    fn stale_filter_deletes_only_missing_loaders_on_our_esp() {
        let entries = parse_efi_entries(LISTING);
        // On our ESP (that GUID) only EFI/archtest/grubx64.efi is missing.
        let exists = |p: &str| p != "efi/archtest/grubx64.efi";
        let stale = stale_boot_entries(
            &entries,
            Some("D26C4B8A-1234-4187-ABCD-00E3A37BD45F"),
            &exists,
        );
        assert_eq!(stale.len(), 1, "{stale:?}");
        assert_eq!(stale[0].num, 5);
        assert_eq!(stale[0].label, "camareLOL-old-test");
    }

    #[test]
    fn stale_filter_never_takes_entries_off_a_foreign_esp() {
        // Same disk, different partition, different GUID: EndeavourOS's
        // entry must survive EVEN WHEN the file check would call it
        // missing — the GUID guard is what keeps us off other people's
        // loaders. USB live media and CD-ROM (no HD descriptor) likewise.
        let entries = parse_efi_entries(LISTING);
        let exists = |_: &str| false; // even when missing on our ESP
        let stale = stale_boot_entries(
            &entries,
            Some("d26c4b8a-1234-4187-abcd-00e3a37bd45f"),
            &exists,
        );
        let mut nums: Vec<u16> = stale.iter().map(|e| e.num).collect();
        nums.sort();
        assert_eq!(nums, [2, 5, 6], "{stale:?}");
        assert!(
            nums.iter().all(|n| n != &7),
            "ESP nước ngoài không được đụng"
        );
    }

    #[test]
    fn parser_survives_a_truncated_or_empty_listing() {
        assert!(parse_efi_entries("").is_empty());
        assert!(parse_efi_entries("BootCurrent: 0002\nTimeout: 0\n").is_empty());
        // a line missing the hex digits entirely
        assert!(parse_efi_entries("BootXXXX* Weird\tHD(1,GPT,x,0,0)").is_empty());
    }

    fn plan_with(purpose: PartitionPurpose, mount: &str) -> PartitionPlan {
        PartitionPlan {
            disk_path: "/dev/vda".into(),
            disk_identity: "test".into(),
            disk_risk: RiskLevel::Low,
            table_type: PartitionTableType::Gpt,
            keep_existing: false,
            partitions: vec![
                PartitionSpec {
                    purpose: PartitionPurpose::EfiSystem,
                    size: SizeSpec::Bytes(1024 * 1024 * 1024),
                    filesystem: Some(Filesystem::Vfat),
                    mount_point: Some("/boot".into()),
                    label: Some("ESP".into()),
                    format: true,
                    number: None,
                    start_sector: None,
                },
                PartitionSpec {
                    purpose,
                    size: SizeSpec::RemainingSpace,
                    filesystem: Some(Filesystem::Ext4),
                    mount_point: Some(mount.into()),
                    label: Some("root".into()),
                    format: true,
                    number: None,
                    start_sector: None,
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
        let err =
            validate_boot_config(&broken, Firmware::Uefi, Bootloader::SystemdBoot).unwrap_err();
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
