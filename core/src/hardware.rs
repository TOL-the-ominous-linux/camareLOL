//! Hardware detection: CPU, RAM, GPU and firmware mode.
//!
//! Everything is read from `/proc`, `/sys` or documented tools; nothing is
//! guessed from model name strings alone.
//!
//! The two tools used here (`lspci`, `systemd-detect-virt`) only *report*, so
//! they go through [`crate::sys::inspect`] — the one path the `--preview`
//! latch still allows. A preview that could not see the GPU would be a poorer
//! preview, and neither tool can change anything.

use crate::sys;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Firmware {
    Uefi,
    Bios,
    Unknown,
}

impl Firmware {
    pub fn as_str(&self) -> &'static str {
        match self {
            Firmware::Uefi => "UEFI",
            Firmware::Bios => "BIOS",
            Firmware::Unknown => "unknown",
        }
    }
}

/// Detect the live environment firmware mode.
pub fn detect_firmware() -> Firmware {
    if sys::path_exists("/sys/firmware/efi/efivars") {
        Firmware::Uefi
    } else if sys::path_exists("/sys/firmware/acpi/tables")
        || sys::path_exists("/sys/firmware/devicetree")
    {
        Firmware::Bios
    } else {
        Firmware::Unknown
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuInfo {
    pub model: String,
    pub cores: usize,
    pub threads: usize,
    /// `GenuineIntel`, `AuthenticAMD`, ...
    pub vendor: String,
    pub microcode_hint: MicrocodeHint,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MicrocodeHint {
    Intel,
    Amd,
    Unknown,
}

pub fn detect_cpu() -> CpuInfo {
    let mut model = String::from("unknown CPU");
    let mut vendor = String::new();
    let mut ids: Vec<(u32, u32)> = Vec::new();
    if let Ok(cpuinfo) = sys::read_file("/proc/cpuinfo") {
        for line in cpuinfo.lines() {
            if line.is_empty() {
                continue;
            }
            let (key, value) = match line.split_once(':') {
                Some((k, v)) => (k.trim(), v.trim()),
                None => continue,
            };
            match key {
                "model name" if model == "unknown CPU" => model = value.to_string(),
                "vendor_id" if vendor.is_empty() => vendor = value.to_string(),
                "processor" => {
                    if let Ok(core) = value.parse::<u32>() {
                        ids.push((core, 0));
                    }
                }
                "core id" => {
                    if let (Some(last), Ok(core)) = (ids.last_mut(), value.parse::<u32>()) {
                        last.1 = core;
                    }
                }
                _ => {}
            }
        }
    }
    let threads = ids.len().max(1);
    let cores = {
        let mut unique: Vec<u32> = ids.iter().map(|(_, c)| *c).collect();
        unique.sort_unstable();
        unique.dedup();
        unique.len().max(1)
    };
    let microcode_hint = if vendor.contains("Intel") || vendor.contains("GenuineIntel") {
        MicrocodeHint::Intel
    } else if vendor.contains("AMD") || vendor.contains("AuthenticAMD") {
        MicrocodeHint::Amd
    } else {
        MicrocodeHint::Unknown
    };
    CpuInfo {
        model,
        cores,
        threads,
        vendor,
        microcode_hint,
    }
}

pub fn detect_ram_bytes() -> u64 {
    if let Ok(meminfo) = sys::read_file("/proc/meminfo") {
        for line in meminfo.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                let kib: u64 = rest
                    .split_whitespace()
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                return kib * 1024;
            }
        }
    }
    0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub description: String,
    pub vendor: GpuVendor,
    /// Product name without the bracketed PCI id (`lspci -nn` puts the id in
    /// square brackets after the name): for "GA107M [GeForce RTX 3050 Mobile]
    /// [10de:25a2]" this is "GA107M [GeForce RTX 3050 Mobile]". Empty when the
    /// field could not be parsed — description still carries everything.
    #[serde(default)]
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GpuVendor {
    Intel,
    Amd,
    Nvidia,
    Vmware,
    Other,
}

/// Split an `lspci -mm -nn` line into its fields: the slot token first, then
/// every quoted string. `"00:02.0" "VGA compatible controller [0300]" "Intel…"`
/// becomes `["00:02.0", "VGA compatible controller [0300]", "Intel…"]`.
/// The trailing `-rNN` revision flags are skipped, not produced.
fn split_lspci_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    // The slot is everything before the first quote; keep it when non-empty.
    if let Some(qpos) = chars.iter().position(|c| *c == '"') {
        let slot: String = chars[..qpos].iter().collect();
        if !slot.trim().is_empty() {
            fields.push(slot.trim().to_string());
        }
        i = qpos;
    }

    // Walk the rest: each quoted string is one field, unquoted runs between
    // quotes (like " -r02") carry no information and are dropped.
    while i < chars.len() {
        if chars[i] == '"' {
            let mut j = i + 1;
            while j < chars.len() && chars[j] != '"' {
                j += 1;
            }
            if j < chars.len() {
                fields.push(chars[i + 1..j].iter().collect());
                i = j + 1;
            } else {
                break;
            }
        } else {
            i += 1;
        }
    }
    fields
}

/// `lspci -nn` writes the PCI id in square brackets after the device name —
/// `[10de:25a2]`, `[8086]`, `[0300]`. Strip only genuine id brackets: the
/// contents must be hex digits and colons, because "[GeForce RTX 3050 Mobile]"
/// is part of the name, not the id.
fn strip_pci_id(name: &str) -> String {
    let trimmed = name.trim();
    let Some(open) = trimmed.rfind('[') else {
        return trimmed.to_string();
    };
    let inner = trimmed[open + 1..].trim_end_matches(']');
    if !inner.is_empty()
        && inner
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == ':' || c == 'x')
        && inner.contains(':')
    {
        return trimmed[..open].trim().to_string();
    }
    trimmed.to_string()
}

/// Best effort GPU discovery via `lspci` when available.
pub fn detect_gpus() -> Vec<GpuInfo> {
    let mut gpus = Vec::new();
    if let Ok(out) = sys::inspect("lspci", &["-mm", "-nn"]) {
        if out.success() {
            for line in out.stdout.lines() {
                if line.to_ascii_lowercase().contains("vga")
                    || line.to_ascii_lowercase().contains("3d controller")
                    || line.to_ascii_lowercase().contains("display")
                {
                    let lower = line.to_ascii_lowercase();
                    let vendor = if lower.contains("nvidia") {
                        GpuVendor::Nvidia
                    } else if lower.contains("intel") {
                        GpuVendor::Intel
                    } else if lower.contains("amd") || lower.contains("ati") {
                        GpuVendor::Amd
                    } else if lower.contains("vmware") {
                        GpuVendor::Vmware
                    } else {
                        GpuVendor::Other
                    };
                    // Quoted fields: [slot, class, vendor, device, …]. The
                    // model is the device name with its PCI id stripped.
                    let fields = split_lspci_fields(line.trim());
                    let model = fields.get(3).map(|f| strip_pci_id(f)).unwrap_or_default();
                    gpus.push(GpuInfo {
                        description: line.trim().to_string(),
                        vendor,
                        model,
                    });
                }
            }
        }
    }
    gpus
}

/// True when we appear to run inside a virtual machine (used by tests).
pub fn running_in_vm() -> bool {
    if let Ok(out) = sys::inspect("systemd-detect-virt", &[]) {
        if out.success() {
            let kind = out.stdout.trim();
            return !kind.is_empty() && kind != "none";
        }
    }
    false
}

/// Complete hardware summary for the welcome / preflight screens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareSummary {
    pub firmware: Firmware,
    pub cpu: CpuInfo,
    pub ram_bytes: u64,
    pub gpus: Vec<GpuInfo>,
    pub virtualized: bool,
}

pub fn summarize() -> HardwareSummary {
    HardwareSummary {
        firmware: detect_firmware(),
        cpu: detect_cpu(),
        ram_bytes: detect_ram_bytes(),
        gpus: detect_gpus(),
        virtualized: running_in_vm(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firmware_detection_is_stable() {
        let a = detect_firmware();
        let b = detect_firmware();
        assert_eq!(a, b);
    }

    #[test]
    fn cpu_detection_returns_something() {
        let cpu = detect_cpu();
        assert!(cpu.threads >= 1);
    }

    #[test]
    fn ram_detection_nonzero_on_linux() {
        // Only meaningful when /proc is available, which it is during tests.
        if std::path::Path::new("/proc/meminfo").exists() {
            assert!(detect_ram_bytes() > 0);
        }
    }

    #[test]
    fn lspci_fields_split_on_quotes() {
        let line = r#"00:02.0 "VGA compatible controller [0300]" "Intel Corporation [8086]" "UHD Graphics 620 [8086:3ea0]" -r02 "MSI [1462]""#;
        let f = split_lspci_fields(line);
        assert_eq!(f[0], "00:02.0");
        assert_eq!(f[1], "VGA compatible controller [0300]");
        assert_eq!(f[3], "UHD Graphics 620 [8086:3ea0]");
        assert_eq!(f.len(), 5, "cờ -r02 phải bị bỏ: {f:?}");
    }

    #[test]
    fn gpu_model_strips_only_the_id_bracket() {
        let line = r#"01:00.0 "3D controller [0302]" "NVIDIA Corporation [10de]" "GA107M [GeForce RTX 3050 Mobile] [10de:25a2]" "MSI [1462]""#;
        let f = split_lspci_fields(line);
        let model = strip_pci_id(&f[3]);
        assert_eq!(
            model, "GA107M [GeForce RTX 3050 Mobile]",
            "dấu ngoặc của tên sản phẩm không phải id — phải giữ lại"
        );
        assert_eq!(strip_pci_id("VGA Device [1002:744c]"), "VGA Device");
        // không có id nào cả — giữ nguyên
        assert_eq!(strip_pci_id("VGA Device"), "VGA Device");
        // ngoặc [trong] tên — id phải có dấu ':', không strip được
        assert_eq!(
            strip_pci_id("GA107M [GeForce RTX 3050]"),
            "GA107M [GeForce RTX 3050]"
        );
    }
}
