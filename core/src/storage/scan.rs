//! Real disk discovery.
//!
//! Sources (in order of trust):
//! 1. `lsblk -J -b -o ...` for the topology and sizes
//! 2. `findmnt` for mount state
//! 3. `blkid` for filesystem/UUID/label enrichment (only when lsblk lacks it)
//!
//! A `--mock` survey (JSON) can replace the real scan, which is how the
//! acceptance tests in the spec (empty disk / Windows disk / partition without
//! filesystem / device disappeared) run without touching real hardware.

use super::device::*;
use crate::error::{CamarelolError, ErrorCode, Result};
use crate::sys;
use serde_json::Value;

/// Run the real scan against this machine.
pub fn scan_devices() -> Result<Vec<StorageDevice>> {
    let out = sys::run(
        "lsblk",
        &[
            "-J", "-b", "-e", "7", // skip loop devices
            "-o",
            "NAME,KNAME,PKNAME,PATH,TYPE,SIZE,FSTYPE,FSVER,LABEL,UUID,PARTUUID,PARTTYPE,PARTTYPENAME,PARTLABEL,PTTYPE,MOUNTPOINT,MOUNTPOINTS,ROTA,RM,TRAN,SERIAL,WWN,MODEL,VENDOR,LOG-SEC,PHY-SEC",
        ],
    )
    .map_err(|_| {
        CamarelolError::new(ErrorCode::MissingTool, "lsblk is not available")
            .with_remediation("install util-linux: pacman -S util-linux")
    })?;
    if !out.success() {
        return Err(
            CamarelolError::new(ErrorCode::CommandFailed, "lsblk failed")
                .with_context(out.stderr.trim().to_string()),
        );
    }
    let mut devices = parse_lsblk_json(&out.stdout)?;
    enrich_mounts(&mut devices)?;
    for d in devices.iter_mut() {
        d.risk = d.assess_risk();
    }
    Ok(devices)
}

/// Parse a device survey previously produced by `--scan` (JSON array).
pub fn scan_devices_from_survey(json: &str) -> Result<Vec<StorageDevice>> {
    // Surveys are produced by serde on our own model, but hand-written surveys
    // (tests, bug reports) may use either scalar or list values.
    let mut value: Value = serde_json::from_str(json).map_err(|e| {
        CamarelolError::new(ErrorCode::ParseFailed, format!("bad survey JSON: {e}"))
    })?;
    normalize_lists(&mut value);
    let devices: Vec<StorageDevice> = serde_json::from_value(value).map_err(|e| {
        CamarelolError::new(ErrorCode::ParseFailed, format!("bad survey JSON: {e}"))
    })?;
    if devices.is_empty() {
        return Err(CamarelolError::new(
            ErrorCode::NoTargetDisk,
            "the device survey is empty",
        ));
    }
    Ok(devices)
}

/// Load a survey from a file (used by tests and `--mock`).
pub fn load_survey(path: &str) -> Result<Vec<StorageDevice>> {
    let raw = sys::read_file(path)?;
    scan_devices_from_survey(&raw)
}

/// Keys whose value may be a JSON array in `lsblk -J` output (and therefore in
/// our surveys): we keep the first element.
const LIST_VALUED_KEYS: &[&str] = &[
    "model",
    "vendor",
    "serial",
    "wwn",
    "label",
    "partlabel",
    "mountpoint",
    "fstype",
    "parttype",
    "parttypename",
    "transport",
];

/// Normalize a survey/lsblk JSON document so list-valued fields become scalars.
fn normalize_lists(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if LIST_VALUED_KEYS.contains(&key.as_str()) {
                    if let Value::Array(arr) = val {
                        let first = arr.first().cloned().unwrap_or(Value::Null);
                        *val = first;
                    }
                }
                normalize_lists(val);
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize_lists(item);
            }
        }
        _ => {}
    }
}

/// Accept a string or a single-element array (lsblk emits arrays for several
/// fields).
fn str_field(v: &Value, key: &str) -> Option<String> {
    match v.get(key) {
        Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
        // lsblk emits numeric fields as JSON numbers (e.g. `log-sec`).
        Some(Value::Number(n)) => Some(n.to_string()),
        Some(Value::Array(arr)) => arr.first().and_then(|x| {
            x.as_str()
                .map(|s| s.to_string())
                .or_else(|| x.as_u64().map(|n| n.to_string()))
        }),
        _ => None,
    }
}

fn u64_field(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(|x| x.as_u64()).unwrap_or(0)
}

fn bool_field(v: &Value, key: &str) -> bool {
    v.get(key).and_then(|x| x.as_bool()).unwrap_or(false)
}

fn classify_partition(p: &mut Partition) {
    if p.is_efi() && !p.os_hints.contains(&OsSignature::EfiSystemPartition) {
        p.os_hints.push(OsSignature::EfiSystemPartition);
    }
    if p.is_microsoft_reserved() && !p.os_hints.contains(&OsSignature::MicrosoftReserved) {
        p.os_hints.push(OsSignature::MicrosoftReserved);
    }
    if p.is_windows_recovery() && !p.os_hints.contains(&OsSignature::WindowsRecovery) {
        p.os_hints.push(OsSignature::WindowsRecovery);
    }
    if p.filesystem == Some(Filesystem::Ntfs)
        && !p.os_hints.contains(&OsSignature::Windows)
        && !p.is_windows_recovery()
    {
        // NTFS without a marker label: could be a data volume of Windows.
        p.os_hints.push(OsSignature::Windows);
    }
    match p.filesystem {
        Some(Filesystem::Swap) if !p.os_hints.contains(&OsSignature::LinuxSwap) => {
            p.os_hints.push(OsSignature::LinuxSwap)
        }
        _ => {}
    }
}

fn device_kind(
    _v: &Value,
    name: &str,
    rotational: bool,
    removable: bool,
    transport: Option<&str>,
) -> DeviceKind {
    match transport {
        Some("usb") => return DeviceKind::Usb,
        Some("nvme") => return DeviceKind::NvmeSsd,
        _ => {}
    }
    if name.starts_with("nvme") {
        DeviceKind::NvmeSsd
    } else if name.starts_with("mmcblk") {
        DeviceKind::Mmc
    } else if removable {
        DeviceKind::Usb
    } else if rotational {
        DeviceKind::SataHdd
    } else if name.starts_with("vd") || name.starts_with("xvd") {
        DeviceKind::Virtual
    } else {
        DeviceKind::SataSsd
    }
}

fn parse_partition(v: &Value) -> Partition {
    let name = str_field(v, "name").unwrap_or_default();
    let path = str_field(v, "path").unwrap_or_else(|| format!("/dev/{name}"));
    let mut p = Partition {
        kernel_name: name,
        path,
        label: str_field(v, "label"),
        uuid: str_field(v, "uuid"),
        part_uuid: str_field(v, "partuuid"),
        size_bytes: u64_field(v, "size"),
        filesystem: str_field(v, "fstype").map(|s| Filesystem::from_str_loose(&s)),
        partition_type: str_field(v, "parttype"),
        partition_type_name: str_field(v, "parttypename"),
        mount_state: MountState::NotMounted,
        os_hints: Vec::new(),
        part_label: str_field(v, "partlabel"),
    };
    classify_partition(&mut p);
    p
}

/// Parse `lsblk -J` output.
pub fn parse_lsblk_json(raw: &str) -> Result<Vec<StorageDevice>> {
    let root: Value = serde_json::from_str(raw).map_err(|e| {
        CamarelolError::new(
            ErrorCode::ParseFailed,
            format!("cannot parse lsblk JSON: {e}"),
        )
    })?;
    let devices = root
        .get("blockdevices")
        .and_then(|b| b.as_array())
        .ok_or_else(|| {
            CamarelolError::new(ErrorCode::ParseFailed, "lsblk JSON has no blockdevices")
        })?;

    let mut out = Vec::new();
    for d in devices {
        let kind_type = str_field(d, "type").unwrap_or_default();
        if kind_type != "disk" {
            // Ignore partitions at the top level (should not happen) and roms.
            continue;
        }
        let mut dev = parse_disk(d)?;
        dev.risk = dev.assess_risk();
        out.push(dev);
    }
    Ok(out)
}

fn parse_disk(d: &Value) -> Result<StorageDevice> {
    let name = str_field(d, "name").unwrap_or_default();
    let path = str_field(d, "path").unwrap_or_else(|| format!("/dev/{name}"));
    let rotational = bool_field(d, "rota");
    let removable = bool_field(d, "rm");
    let transport = str_field(d, "tran");
    let internal_kind = device_kind(d, &name, rotational, removable, transport.as_deref());
    let pt = match str_field(d, "pttype").as_deref() {
        Some("gpt") => PartitionTableType::Gpt,
        Some("dos") | Some("mbr") => PartitionTableType::Mbr,
        Some(_) => PartitionTableType::Unknown,
        None => PartitionTableType::None,
    };

    let mut partitions = Vec::new();
    if let Some(children) = d.get("children").and_then(|c| c.as_array()) {
        for c in children {
            partitions.push(parse_partition(c));
        }
    }
    // A partition table with zero partitions still is a table.
    let pt_type = if partitions.is_empty() && pt == PartitionTableType::None {
        PartitionTableType::None
    } else if pt != PartitionTableType::None {
        pt
    } else {
        PartitionTableType::Unknown
    };

    let mut device = StorageDevice {
        kernel_name: name,
        path,
        model: str_field(d, "model"),
        vendor: str_field(d, "vendor"),
        serial: str_field(d, "serial"),
        wwn: str_field(d, "wwn"),
        size_bytes: u64_field(d, "size"),
        logical_sector_size: str_field(d, "log-sec").and_then(|s| s.parse().ok()),
        physical_sector_size: str_field(d, "phy-sec").and_then(|s| s.parse().ok()),
        rotational,
        removable,
        transport,
        kind: internal_kind,
        partition_table: pt_type,
        partitions,
        mount_state: MountState::NotMounted,
        os_hints: Vec::new(),
        encrypted: None,
        risk: RiskLevel::Low,
    };
    device.risk = device.assess_risk();
    Ok(device)
}

/// Fill in mount state using `findmnt`.
///
/// Any mount *not* under `/mnt` is considered part of the live system, because
/// camareLOL runs from the installation medium.
pub fn enrich_mounts(devices: &mut [StorageDevice]) -> Result<()> {
    let targets: Vec<(String, String)> = match sys::run("findmnt", &["-rn", "-o", "SOURCE,TARGET"])
    {
        Ok(out) if out.success() => out
            .stdout
            .lines()
            .filter_map(|l| {
                let mut it = l.split_whitespace();
                let src = it.next()?.to_string();
                let tgt = it.next()?.to_string();
                Some((src, tgt))
            })
            .collect(),
        _ => Vec::new(),
    };

    fn resolve<'a>(source: &str, targets: &'a [(String, String)]) -> Option<&'a str> {
        let by_path = targets.iter().find(|(s, _)| s == source);
        by_path.map(|(_, t)| t.as_str()).or_else(|| {
            // findmnt reports /dev/mapper/... for LUKS; also accept name match.
            let name = source.rsplit('/').next().unwrap_or(source);
            targets
                .iter()
                .find(|(s, _)| s.ends_with(&format!("/{name}")))
                .map(|(_, t)| t.as_str())
        })
    }

    for dev in devices.iter_mut() {
        if let Some(target) = resolve(&dev.path, &targets) {
            dev.mount_state = MountState::Mounted {
                target: target.to_string(),
                live_system: !target.starts_with("/mnt"),
            };
        }
        for p in dev.partitions.iter_mut() {
            if let Some(target) = resolve(&p.path, &targets) {
                p.mount_state = MountState::Mounted {
                    target: target.to_string(),
                    live_system: !target.starts_with("/mnt"),
                };
            }
        }
    }
    Ok(())
}

/// Verify a disk reference against a known set of devices.
///
/// Matches by identity first (WWN/serial) and only falls back to the device
/// path when the identity cannot be compared at all. Any mismatch is a hard
/// error — camareLOL never writes to a disk it cannot positively identify.
pub fn verify_against(
    path_hint: &str,
    expected_identity: &str,
    expected_size: u64,
    devices: &[StorageDevice],
) -> Result<StorageDevice> {
    let by_identity = devices
        .iter()
        .find(|d| d.identity() == expected_identity)
        .cloned();
    let chosen = match by_identity {
        Some(d) => d,
        None => {
            // Only accept the path when every device lacks an identity.
            let any_identity = devices
                .iter()
                .any(|d| d.identity_strength() > IdentityStrength::KernelNameOnly);
            if any_identity {
                return Err(CamarelolError::new(
                    ErrorCode::DeviceIdentityChanged,
                    format!("no device with identity `{expected_identity}` is present"),
                )
                .with_remediation(
                    "the disk order changed since the plan was created; redo the plan",
                ));
            }
            devices
                .iter()
                .find(|d| d.path == path_hint)
                .cloned()
                .ok_or_else(|| {
                    CamarelolError::new(
                        ErrorCode::DeviceGone,
                        format!("device {path_hint} is not present"),
                    )
                })?
        }
    };
    if chosen.size_bytes != expected_size {
        return Err(CamarelolError::new(
            ErrorCode::DeviceIdentityChanged,
            format!("size of {} changed", chosen.path),
        ));
    }
    Ok(chosen)
}

/// Re-scan just one device and confirm it still matches the recorded identity.
pub fn rescan_and_verify(expected: &StorageDevice) -> Result<StorageDevice> {
    let devices = scan_devices()?;
    let found = devices
        .into_iter()
        .find(|d| d.path == expected.path)
        .ok_or_else(|| {
            CamarelolError::new(
                ErrorCode::DeviceGone,
                format!("device {} is not present any more", expected.path),
            )
            .with_remediation("go back to the disk selection step and choose a disk again")
        })?;

    if found.identity() != expected.identity() {
        return Err(CamarelolError::new(
            ErrorCode::DeviceIdentityChanged,
            format!(
                "identity of {} changed (was `{}`, now `{}`)",
                expected.path,
                expected.identity(),
                found.identity()
            ),
        )
        .with_remediation("hardware topology changed since the plan was created; redo the plan"));
    }
    if found.size_bytes != expected.size_bytes {
        return Err(CamarelolError::new(
            ErrorCode::DeviceIdentityChanged,
            format!("size of {} changed", expected.path),
        ));
    }
    if found.partitions.len() != expected.partitions.len()
        || found.partition_table != expected.partition_table
    {
        return Err(CamarelolError::new(
            ErrorCode::DeviceIdentityChanged,
            format!(
                "layout of {} changed since the plan was created (plan: {} partitions, now: {})",
                expected.path,
                expected.partitions.len(),
                found.partitions.len()
            ),
        )
        .with_remediation("re-scan the disk and confirm the plan again"));
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SURVEY_EMPTY: &str = r#"[{"kernel_name":"sda","path":"/dev/sda","model":["QEMU HARDDISK"],"vendor":null,"serial":null,"wwn":null,"size_bytes":21474836480,"logical_sector_size":512,"physical_sector_size":512,"rotational":true,"removable":false,"transport":null,"kind":"SataHdd","partition_table":"None","partitions":[],"mount_state":"NotMounted","os_hints":[],"encrypted":null,"risk":"Low"}]"#;

    #[test]
    fn parses_survey_of_empty_disk() {
        let d = scan_devices_from_survey(SURVEY_EMPTY).unwrap();
        assert_eq!(d.len(), 1);
        assert!(d[0].is_empty());
        assert_eq!(d[0].assess_risk(), RiskLevel::Low);
    }

    #[test]
    fn rejects_empty_survey() {
        assert!(scan_devices_from_survey("[]").is_err());
    }

    #[test]
    fn parses_lsblk_minimal_json() {
        let raw = r#"{"blockdevices":[{"name":"nvme0n1","type":"disk","size":500107862016,"rota":false,"rm":false,"tran":"nvme","pttype":"gpt","children":[{"name":"nvme0n1p1","type":"part","size":1048576,"fstype":"vfat","parttype":"c12a7328-f81f-11d2-ba4b-00a0c93ec93b"},{"name":"nvme0n1p3","type":"part","size":998579896320,"fstype":"ntfs"}]}]}"#;
        let disks = parse_lsblk_json(raw).unwrap();
        assert_eq!(disks.len(), 1);
        let d = &disks[0];
        assert_eq!(d.kind, DeviceKind::NvmeSsd);
        assert_eq!(d.partition_table, PartitionTableType::Gpt);
        assert_eq!(d.partitions.len(), 2);
        assert!(d.partitions[0].is_efi());
        assert!(d.has_windows());
        assert_eq!(d.assess_risk(), RiskLevel::High);
        assert_eq!(d.identity_strength(), IdentityStrength::KernelNameOnly);
    }

    #[test]
    fn partition_without_filesystem_is_medium_risk() {
        let raw = r#"{"blockdevices":[{"name":"vdb","type":"disk","size":10737418240,"rota":false,"rm":false,"pttype":"gpt","children":[{"name":"vdb1","type":"part","size":10737418240}]}]}"#;
        let disks = parse_lsblk_json(raw).unwrap();
        assert_eq!(disks.len(), 1);
        assert_eq!(disks[0].partitions.len(), 1);
        assert!(!disks[0].partitions[0].has_filesystem());
        assert_eq!(disks[0].assess_risk(), RiskLevel::Medium);
        assert_eq!(disks[0].kind, DeviceKind::Virtual);
    }

    #[test]
    fn real_scan_on_test_host_works_when_lsblk_present() {
        if crate::sys::tool_available("lsblk") {
            let devices = scan_devices().unwrap();
            for d in devices {
                assert_eq!(d.risk, d.assess_risk());
            }
        }
    }
}
