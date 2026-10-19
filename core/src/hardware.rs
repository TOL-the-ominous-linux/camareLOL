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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GpuVendor {
    Intel,
    Amd,
    Nvidia,
    Vmware,
    Other,
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
                    gpus.push(GpuInfo {
                        description: line.trim().to_string(),
                        vendor,
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
}
