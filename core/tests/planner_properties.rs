//! Property tests for the partition planner.
//!
//! The planner is the one component that can destroy a disk. The unit tests
//! pin the cases we already know; these generate thousands of devices and
//! requests and assert the invariants that must hold for *every* plan:
//!
//! * a whole-disk plan accounts for every byte of the disk exactly once;
//! * a plan the planner returns has valid geometry: `validate_plan` may still
//!   refuse the *boot configuration* the caller asked for, never the layout;
//! * a dual-boot plan preserves every existing partition, never formats it,
//!   and places every new partition inside the free gap without overlapping;
//! * the gap scan never reports occupied space as free.
//!
//! The planner is a layout generator; the boot configuration is validated by
//! `validate_plan`/`validate_boot_config` before anything is written (the
//! install preflight does exactly that). These tests therefore assert the
//! geometry of every plan, including plans whose boot configuration a caller
//! would later refuse — a plan that is safe to draw must never be unsafe to
//! write.
//!
//! The generator is deterministic: a failure prints the seed, and
//! `CAMARELOL_PROP_SEED` / `CAMARELOL_PROP_CASES` reproduce or widen a run.
//! That keeps this dependency-free: a project that ships a disk formatter
//! should not grow a property-testing framework to test it.
//!
//! `START` in `lsblk` is always in 512-byte sectors (see
//! `lsblk --list-columns`), regardless of the device's logical sector size.
//! The generator models that faithfully: a 4K-native disk still reports its
//! partition starts in 512-byte units.

use camarelol_core::boot::Bootloader;
use camarelol_core::error::ErrorCode;
use camarelol_core::hardware::Firmware;
use camarelol_core::storage::device::{
    DeviceKind, Filesystem, MountState, Partition, PartitionTableType, RiskLevel, StorageDevice,
};
use camarelol_core::storage::layout::{
    largest_aligned_gap, plan_auto_layout, plan_dual_boot, validate_plan, GapScan, LayoutRequest,
    PartitionPurpose, PartitionSpec, SizeSpec,
};
use std::panic::{catch_unwind, AssertUnwindSafe};

const GIB: u64 = 1024 * 1024 * 1024;
const MIB: u64 = 1024 * 1024;

/// `lsblk` START and the sfdisk/sgdisk paths are all in 512-byte sectors.
const SECTOR: u64 = 512;
/// 1 MiB, in 512-byte sectors.
const ALIGN: u64 = 2048;

const GPT_EFI_TYPE: &str = "c12a7328-f81f-11d2-ba4b-00a0c93ec93b";
const GPT_MSR_TYPE: &str = "e3c9e316-0b5c-4db8-817d-f92df00215ae";

/// A small, deterministic generator. xorshift64* is enough here: the goal is
/// reproducible coverage, not cryptographic quality.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // splitmix64 seeding, so seed 0 is not a fixed point.
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        Rng(z | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next() % n
        }
    }

    fn bool(&mut self) -> bool {
        self.next() & 1 == 1
    }

    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[self.below(xs.len() as u64) as usize]
    }
}

struct Case {
    device: StorageDevice,
    req: LayoutRequest,
}

fn device_with(
    size_bytes: u64,
    logical_sector_size: u32,
    table: PartitionTableType,
    partitions: Vec<Partition>,
) -> StorageDevice {
    StorageDevice {
        kernel_name: "nvme0n1".into(),
        path: "/dev/nvme0n1".into(),
        model: None,
        vendor: None,
        serial: Some("property-test".into()),
        wwn: None,
        size_bytes,
        logical_sector_size: Some(logical_sector_size),
        physical_sector_size: Some(logical_sector_size),
        rotational: false,
        removable: false,
        transport: None,
        kind: DeviceKind::Virtual,
        partition_table: table,
        partitions,
        mount_state: MountState::NotMounted,
        os_hints: vec![],
        encrypted: None,
        risk: RiskLevel::Low,
    }
}

fn random_request(rng: &mut Rng) -> LayoutRequest {
    LayoutRequest {
        firmware: *rng.pick(&[Firmware::Uefi, Firmware::Bios, Firmware::Unknown]),
        bootloader: *rng.pick(&[Bootloader::Grub, Bootloader::SystemdBoot]),
        root_filesystem: rng
            .pick(&[
                Filesystem::Ext4,
                Filesystem::Btrfs,
                Filesystem::Xfs,
                Filesystem::Vfat,
                Filesystem::Ntfs,
                Filesystem::Swap,
                Filesystem::Luks,
            ])
            .clone(),
        separate_home: rng.bool(),
        separate_boot: rng.bool(),
        swap_bytes: match rng.below(6) {
            0 => None,
            1 => Some(0),
            2 => Some(512 * MIB),
            3 => Some(4 * GIB),
            4 => Some(rng.below(64 * GIB)),
            // Deliberately not sector-aligned: the planner must not lose bytes.
            _ => Some(1),
        },
        encrypt_root: rng.bool(),
    }
}

fn auto_case(seed: u64) -> Case {
    let mut rng = Rng::new(seed);
    let size = match rng.below(10) {
        0 => 0,
        1 => MIB,
        2 => 8 * GIB - 1,
        3 => 8 * GIB,
        4 => 8 * GIB + MIB,
        5 => 20 * GIB,
        6 => 40 * GIB + 12_345,
        7 => 512 * MIB,
        8 => rng.below(200 * GIB),
        _ => 2 * 1024 * GIB,
    };
    let logical = *rng.pick(&[512u32, 512, 4096, 0, 1000]);
    let table = rng
        .pick(&[
            PartitionTableType::None,
            PartitionTableType::Gpt,
            PartitionTableType::Mbr,
            PartitionTableType::Unknown,
        ])
        .clone();
    Case {
        device: device_with(size, logical, table, vec![]),
        req: random_request(&mut rng),
    }
}

fn partition(
    number: u32,
    start: u64,
    sectors: u64,
    filesystem: Option<Filesystem>,
    partition_type: Option<&str>,
    partition_type_name: Option<&str>,
) -> Partition {
    Partition {
        kernel_name: format!("nvme0n1p{number}"),
        path: format!("/dev/nvme0n1p{number}"),
        label: None,
        uuid: None,
        part_uuid: None,
        // The model stores byte sizes; START stays in 512-byte sectors.
        size_bytes: sectors * SECTOR,
        start_sector: start,
        filesystem,
        partition_type: partition_type.map(str::to_string),
        partition_type_name: partition_type_name.map(str::to_string),
        mount_state: MountState::NotMounted,
        os_hints: vec![],
        part_label: None,
    }
}

/// A Windows-shaped disk with a random free tail (and sometimes a middle gap),
/// plus the occasional broken survey: no starts, a mounted partition, an MBR
/// table, or no ESP.
fn dual_case(seed: u64) -> Case {
    let mut rng = Rng::new(seed);
    let logical = *rng.pick(&[512u32, 512, 4096]);
    let total_sectors = (20 + rng.below(100)) * GIB / SECTOR;
    let mut cursor = ALIGN;
    let mut partitions = Vec::new();
    let mut number = 1u32;

    // The ESP Windows actually has. Missing in 1/10 cases.
    if rng.below(10) != 0 {
        let sectors = (100 + rng.below(900)) * MIB / SECTOR;
        partitions.push(partition(
            number,
            cursor,
            sectors,
            Some(Filesystem::Vfat),
            Some(GPT_EFI_TYPE),
            None,
        ));
        cursor += sectors;
        number += 1;
    }
    // Microsoft Reserved.
    if rng.bool() {
        let sectors = 16 * MIB / SECTOR;
        partitions.push(partition(
            number,
            cursor,
            sectors,
            None,
            Some(GPT_MSR_TYPE),
            None,
        ));
        cursor += sectors;
        number += 1;
    }
    // Free space a deleted/shrunk partition leaves in the middle.
    if rng.bool() {
        cursor += rng.below(8 * GIB / SECTOR) / ALIGN * ALIGN;
    }
    // Windows C: and maybe a data partition.
    for _ in 0..rng.below(3) {
        let sectors = (2 + rng.below(30)) * GIB / SECTOR;
        // Leave at least the minimum root plus fixed pieces for the tail.
        if cursor + sectors + 8 * GIB / SECTOR >= total_sectors {
            break;
        }
        partitions.push(partition(
            number,
            cursor,
            sectors,
            Some(Filesystem::Ntfs),
            None,
            None,
        ));
        cursor += sectors;
        number += 1;
        if rng.bool() {
            cursor += rng.below(4 * GIB / SECTOR) / ALIGN * ALIGN;
        }
    }
    // Recovery partition at the very end.
    if rng.bool() {
        let sectors = GIB / SECTOR;
        if cursor + sectors < total_sectors {
            partitions.push(partition(
                number,
                cursor,
                sectors,
                Some(Filesystem::Ntfs),
                None,
                Some("Windows recovery"),
            ));
            cursor += sectors;
        }
    }
    // The free tail: sometimes absent, sometimes enough, sometimes huge.
    let tail = match rng.below(5) {
        0 => 0,
        1 => (1 + rng.below(4)) * GIB / SECTOR,
        2 => (8 + rng.below(8)) * GIB / SECTOR,
        3 => (16 + rng.below(64)) * GIB / SECTOR,
        _ => rng.below(100 * GIB) / SECTOR,
    };
    let size_bytes = (cursor + tail) * SECTOR;

    // A broken survey must be refused, not misread.
    if rng.below(20) == 0 {
        for p in &mut partitions {
            p.start_sector = 0;
        }
    }
    if rng.below(20) == 0 {
        if let Some(p) = partitions.last_mut() {
            p.mount_state = MountState::Mounted {
                target: "/target".into(),
                live_system: false,
            };
        }
    }
    let table = match rng.below(10) {
        0 => PartitionTableType::Mbr,
        1 => PartitionTableType::None,
        _ => PartitionTableType::Gpt,
    };

    Case {
        device: device_with(size_bytes, logical, table, partitions),
        req: random_request(&mut rng),
    }
}

fn run_cases(name: &str, default_cases: u64, f: impl Fn(u64)) {
    let base: u64 = std::env::var("CAMARELOL_PROP_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0xCA4A_3E10);
    let cases: u64 = std::env::var("CAMARELOL_PROP_CASES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(default_cases);
    for i in 0..cases {
        let seed = base.wrapping_add(i);
        let result = catch_unwind(AssertUnwindSafe(|| f(seed)));
        if let Err(payload) = result {
            let message = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "<non-string panic>".into());
            panic!(
                "{name} failed at seed {seed}: {message}\n\
                 re-run with CAMARELOL_PROP_SEED={seed} CAMARELOL_PROP_CASES=1"
            );
        }
    }
}

fn check_auto(seed: u64) {
    let case = auto_case(seed);
    match plan_auto_layout(&case.device, &case.req) {
        Ok(plan) => {
            // Geometry must be valid for every plan. A caller-chosen boot
            // configuration the planner cannot satisfy is refused here (and
            // again by `validate_boot_config` in the preflight), but that is a
            // refusal of the request, not of the layout.
            if let Err(e) = validate_plan(&plan, &case.req) {
                assert_eq!(
                    e.code,
                    ErrorCode::InvalidBootConfig,
                    "seed {seed}: planner produced a plan with invalid geometry: {e:?}"
                );
            }
            assert_eq!(plan.disk_path, case.device.path);
            assert!(!plan.keep_existing);
            assert_eq!(
                plan.partitions
                    .iter()
                    .filter(|p| p.purpose == PartitionPurpose::Root)
                    .count(),
                1,
                "seed {seed}: whole-disk plan needs exactly one root"
            );

            let mut total = 0u64;
            for p in &plan.partitions {
                match &p.size {
                    SizeSpec::Bytes(b) => {
                        assert!(*b > 0, "seed {seed}: zero-sized partition {p:?}");
                        total += *b;
                    }
                    other => panic!("seed {seed}: whole-disk plan used {other:?}"),
                }
                if p.format {
                    assert!(
                        p.filesystem.is_some(),
                        "seed {seed}: formatted partition without a filesystem: {p:?}"
                    );
                }
                if p.filesystem.as_ref() == Some(&Filesystem::Vfat) {
                    let label = p.label.as_deref().unwrap_or("");
                    assert!(
                        label.len() <= 11,
                        "seed {seed}: FAT label {label:?} is longer than 11 characters"
                    );
                }
            }
            assert_eq!(
                total, case.device.size_bytes,
                "seed {seed}: plan does not account for every byte"
            );

            let destructive = plan.destructive_operations();
            assert_eq!(
                destructive.len(),
                1 + plan.partitions.iter().filter(|p| p.format).count(),
                "seed {seed}: destructive operations do not match the plan"
            );

            match case.req.firmware {
                Firmware::Uefi => assert!(
                    plan.partitions
                        .iter()
                        .any(|p| p.purpose == PartitionPurpose::EfiSystem),
                    "seed {seed}: UEFI plan without an ESP"
                ),
                Firmware::Bios => assert!(
                    !plan
                        .partitions
                        .iter()
                        .any(|p| p.purpose == PartitionPurpose::EfiSystem),
                    "seed {seed}: BIOS plan with an ESP"
                ),
                // Unknown firmware is refused by validate_boot_config before
                // the write path; the layout itself must still be sound.
                Firmware::Unknown => {}
            }
        }
        Err(e) => {
            assert!(!e.message.is_empty(), "seed {seed}: empty error");
            assert!(
                matches!(
                    e.code,
                    ErrorCode::InvalidPlan | ErrorCode::InvalidBootConfig
                ),
                "seed {seed}: unexpected error code {:?}",
                e.code
            );
        }
    }
}

fn check_dual(seed: u64) {
    let case = dual_case(seed);
    let plan = match plan_dual_boot(&case.device, &case.req) {
        Ok(plan) => plan,
        Err(e) => {
            assert!(!e.message.is_empty(), "seed {seed}: empty error");
            assert!(
                matches!(
                    e.code,
                    ErrorCode::InvalidPlan | ErrorCode::InvalidBootConfig
                ),
                "seed {seed}: unexpected error code {:?}",
                e.code
            );
            return;
        }
    };

    if let Err(e) = validate_plan(&plan, &case.req) {
        assert_eq!(
            e.code,
            ErrorCode::InvalidBootConfig,
            "seed {seed}: planner produced a plan with invalid geometry: {e:?}"
        );
    }
    assert!(
        plan.keep_existing,
        "seed {seed}: dual-boot must keep existing"
    );

    // Every existing partition is present exactly once and untouched.
    for p in &case.device.partitions {
        let number = p.number().expect("the generator numbers every partition");
        let matches: Vec<&PartitionSpec> = plan
            .partitions
            .iter()
            .filter(|s| s.number == Some(number))
            .collect();
        assert_eq!(
            matches.len(),
            1,
            "seed {seed}: existing partition {number} is not preserved exactly once"
        );
        let kept = matches[0];
        assert!(
            !kept.format,
            "seed {seed}: existing partition {number} would be formatted"
        );
        assert_eq!(
            kept.size,
            SizeSpec::Bytes(p.size_bytes),
            "seed {seed}: existing partition {number} changed size"
        );
        assert!(
            matches!(
                kept.purpose,
                PartitionPurpose::Keep | PartitionPurpose::EfiSystem
            ),
            "seed {seed}: existing partition {number} changed purpose: {:?}",
            kept.purpose
        );
    }

    let esp = plan
        .partitions
        .iter()
        .find(|s| s.purpose == PartitionPurpose::EfiSystem)
        .expect("a dual-boot plan always reuses an ESP");
    assert!(
        !esp.format,
        "seed {seed}: the reused ESP would be formatted"
    );
    assert_eq!(esp.mount_point.as_deref(), Some("/boot/efi"));

    let (gap_start, gap_end) = match largest_aligned_gap(&case.device) {
        GapScan::Found(start, end) => (start, end),
        other => panic!("seed {seed}: a plan exists but the gap scan says {other:?}"),
    };
    let max_existing = case
        .device
        .partitions
        .iter()
        .filter_map(|p| p.number())
        .max()
        .unwrap_or(0);

    let mut ranges = Vec::new();
    let mut roots = 0;
    for spec in plan.partitions.iter().filter(|s| s.format) {
        let number = spec.number.expect("new partitions carry a number");
        assert!(
            number > max_existing,
            "seed {seed}: new partition number {number} collides with an existing one"
        );
        let start = spec.start_sector.expect("new partitions carry a start");
        let sectors = spec.size.sectors(SECTOR);
        assert!(sectors > 0, "seed {seed}: zero-sized new partition");
        assert!(
            start >= gap_start && start + sectors <= gap_end,
            "seed {seed}: new partition [{start},{}) is outside the gap [{gap_start},{gap_end})",
            start + sectors
        );
        ranges.push((start, start + sectors));
        if spec.purpose == PartitionPurpose::Root {
            roots += 1;
        }
    }
    assert_eq!(
        roots, 1,
        "seed {seed}: dual-boot needs exactly one new root"
    );

    ranges.sort_unstable();
    for pair in ranges.windows(2) {
        assert!(
            pair[0].1 <= pair[1].0,
            "seed {seed}: new partitions overlap: {:?}",
            pair
        );
    }
}

fn check_gap(seed: u64) {
    let case = dual_case(seed);
    let device = &case.device;
    let total_sectors = device.size_bytes / SECTOR;
    let extents: Vec<(u64, u64)> = device
        .partitions
        .iter()
        .filter(|p| p.start_sector > 0 && p.size_bytes > 0)
        .map(|p| (p.start_sector, p.start_sector + p.size_bytes / SECTOR))
        .collect();

    match largest_aligned_gap(device) {
        GapScan::Found(start, end) => {
            assert_eq!(start % ALIGN, 0, "seed {seed}: gap is not 1 MiB aligned");
            assert!(
                start < end && end <= total_sectors,
                "seed {seed}: gap [{start},{end}) is outside the disk ({total_sectors} sectors)"
            );
            for (occupied_start, occupied_end) in &extents {
                assert!(
                    end <= *occupied_start || start >= *occupied_end,
                    "seed {seed}: gap [{start},{end}) overlaps [{occupied_start},{occupied_end})"
                );
            }
        }
        GapScan::NoStarts => assert!(
            extents.is_empty(),
            "seed {seed}: NoStarts with {} readable extents",
            extents.len()
        ),
        GapScan::NoGap => {
            // Nothing to assert beyond "it did not panic"; the planner's own
            // refusal for this device is covered by check_dual.
        }
    }
}

#[test]
fn auto_layout_accounts_for_every_byte() {
    run_cases("auto_layout_accounts_for_every_byte", 600, check_auto);
}

#[test]
fn dual_boot_preserves_every_existing_partition() {
    run_cases(
        "dual_boot_preserves_every_existing_partition",
        600,
        check_dual,
    );
}

#[test]
fn gap_scan_never_returns_occupied_space() {
    run_cases("gap_scan_never_returns_occupied_space", 600, check_gap);
}
