//! System services: audio, bluetooth, network, zram and friends.

use crate::error::{CamarelolError, ErrorCode, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ServiceSelection {
    pub network_manager: bool,
    pub bluetooth: bool,
    pub audio: AudioStack,
    pub printing: bool,
    pub ssh: bool,
    pub zram: bool,
    pub zram_size_mb: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AudioStack {
    None,
    PipeWire,
    PulseAudio,
}

impl AudioStack {
    pub fn as_str(&self) -> &'static str {
        match self {
            AudioStack::None => "none",
            AudioStack::PipeWire => "pipewire",
            AudioStack::PulseAudio => "pulseaudio",
        }
    }
}

impl Default for ServiceSelection {
    fn default() -> Self {
        ServiceSelection {
            network_manager: true,
            bluetooth: false,
            audio: AudioStack::PipeWire,
            printing: false,
            ssh: false,
            zram: true,
            zram_size_mb: None,
        }
    }
}

impl ServiceSelection {
    /// systemd services to enable in the target system.
    pub fn services(&self) -> Vec<String> {
        let mut v = Vec::new();
        if self.network_manager {
            v.push("NetworkManager".into());
        }
        if self.bluetooth {
            v.push("bluetooth".into());
        }
        if self.ssh {
            v.push("sshd".into());
        }
        if self.zram {
            v.push("zram-config".into());
        }
        v
    }

    /// Packages implied by the selection.
    pub fn packages(&self) -> Vec<String> {
        let mut v = Vec::new();
        match self.audio {
            AudioStack::None => {}
            AudioStack::PipeWire => {
                v.push("pipewire".into());
                v.push("pipewire-pulse".into());
                v.push("pipewire-alsa".into());
                v.push("wireplumber".into());
            }
            AudioStack::PulseAudio => {
                v.push("pulseaudio".into());
                v.push("pulseaudio-alsa".into());
            }
        }
        if self.bluetooth {
            v.push("bluez".into());
            v.push("bluez-utils".into());
        }
        if self.zram {
            v.push("zram-generator".into());
        }
        if self.printing {
            v.push("cups".into());
        }
        v
    }

    /// Suggest a zram size from the available RAM (50% rounded to 128 MiB).
    pub fn suggest_zram_mb(ram_bytes: u64) -> u32 {
        let gib = ram_bytes / (1024 * 1024 * 1024);
        let target = gib.saturating_mul(1024) / 2;
        let rounded = target.next_multiple_of(128).max(256);
        rounded as u32
    }
}

/// Validation: enabling a hardware service without hardware is confusing but not
/// fatal — we only warn. Enabling nothing at all is fine too.
pub fn validate(sel: &ServiceSelection, has_bluetooth_hardware: bool) -> Result<()> {
    if sel.bluetooth && !has_bluetooth_hardware {
        return Err(CamarelolError::new(
            ErrorCode::InvalidConfig,
            "Bluetooth was enabled but no Bluetooth adapter was detected",
        )
        .with_remediation("disable the bluetooth service or plug in an adapter"));
    }
    if sel.zram && sel.zram_size_mb == Some(0) {
        return Err(CamarelolError::new(
            ErrorCode::InvalidConfig,
            "zram size must be greater than 0",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_selection_enables_network_and_pipewire() {
        let sel = ServiceSelection::default();
        assert!(sel.services().contains(&"NetworkManager".to_string()));
        assert!(sel.packages().contains(&"pipewire".to_string()));
    }

    #[test]
    fn bluetooth_without_hardware_is_rejected() {
        let mut sel = ServiceSelection::default();
        sel.bluetooth = true;
        assert!(validate(&sel, false).is_err());
        assert!(validate(&sel, true).is_ok());
    }

    #[test]
    fn zram_suggestion_scales_with_ram() {
        let mb = ServiceSelection::suggest_zram_mb(8 * 1024 * 1024 * 1024);
        assert_eq!(mb, 4096);
        let small = ServiceSelection::suggest_zram_mb(512 * 1024 * 1024);
        assert_eq!(small, 256);
    }
}
