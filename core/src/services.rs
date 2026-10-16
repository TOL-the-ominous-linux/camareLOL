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

/// Lower-case on the wire so profiles read `audio = "pipewire"`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AudioStack {
    None,
    #[serde(rename = "pipewire")]
    PipeWire,
    #[serde(rename = "pulseaudio")]
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
            // `zram-generator` — the package `packages()` installs — ships
            // `systemd-zram-setup@.service`, a template instantiated as
            // `systemd-zram-setup@zram0.service`. It does NOT ship the
            // `systemd-zram-generator@zram0.service` of Arch's separate
            // `systemd-zram-generator` package.
            //
            // Enabling a unit that is not installed fails quietly: the install
            // completes, reports "completed with warnings", and zram is simply
            // never on. That is what a VirtualBox install showed: the finished
            // system had `systemd-zram-setup@.service` on disk and no
            // `systemd-zram-generator@zram0.service` to enable.
            v.push("systemd-zram-setup@zram0.service".into());
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
        if self.ssh {
            // Enabling `sshd` without installing openssh fails the same quiet
            // way the zram unit did: the install finishes, warns that the unit
            // is missing, and ssh is not running.
            v.push("openssh".into());
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

    /// Same, in bytes.
    pub fn suggest_zram_bytes(ram_bytes: u64) -> u64 {
        ServiceSelection::suggest_zram_mb(ram_bytes) as u64 * 1024 * 1024
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

    /// Every unit `services()` enables must come from a package
    /// `packages()` installs.
    ///
    /// Enabling a unit that is not on disk is not an error the installer can
    /// see: it warns, carries on, and reports "completed with warnings". The
    /// feature is then quietly absent from a system that claims to have it —
    /// which is how `ssh` shipped without `openssh` and zram shipped with the
    /// wrong unit name.
    ///
    /// `NetworkManager` is excluded on purpose: it comes from the profile's own
    /// core package list, not from this module.
    #[test]
    fn no_unit_is_enabled_without_the_package_that_provides_it() {
        let sel = ServiceSelection {
            network_manager: false,
            bluetooth: true,
            ssh: true,
            zram: true,
            printing: true,
            ..Default::default()
        };
        let pkgs = sel.packages();

        for unit in sel.services() {
            let provider = match unit.as_str() {
                "bluetooth" => "bluez",
                "sshd" => "openssh",
                "systemd-zram-setup@zram0.service" => "zram-generator",
                other => panic!("thêm unit mới {other} thì thêm luôn gói cung cấp nó"),
            };
            assert!(
                pkgs.iter().any(|p| p == provider),
                "bật {unit} nhưng không cài {provider}; packages() = {pkgs:?}"
            );
        }
    }

    #[test]
    fn default_selection_enables_network_and_pipewire() {
        let sel = ServiceSelection::default();
        assert!(sel.services().contains(&"NetworkManager".to_string()));
        assert!(sel.packages().contains(&"pipewire".to_string()));
    }

    /// The zram unit and the zram package have to come from the same project.
    ///
    /// Arch ships two of them and they are not interchangeable: the package
    /// `zram-generator` installs `systemd-zram-setup@zram0.service`, while the
    /// separate `systemd-zram-generator` package installs
    /// `systemd-zram-generator@zram0.service`. Enabling the wrong one fails
    /// quietly — the install reports "completed with warnings" and zram never
    /// comes up, which is exactly how it reached a finished VirtualBox install
    /// before this was pinned.
    #[test]
    fn the_zram_unit_belongs_to_the_zram_package_we_install() {
        let sel = ServiceSelection {
            zram: true,
            ..Default::default()
        };
        assert!(
            sel.packages().contains(&"zram-generator".to_string()),
            "cài zram phải kéo gói zram-generator"
        );
        let units = sel.services();
        let zram = units
            .iter()
            .find(|u| u.contains("zram"))
            .expect("bật zram thì phải có một unit");
        assert!(
            zram.starts_with("systemd-zram-setup@"),
            "unit {zram} không thuộc gói zram-generator — \
             nó thuộc gói systemd-zram-generator, mà ta không cài"
        );
        assert!(
            !units.contains(&"systemd-zram-generator@zram0.service".to_string()),
            "unit này không tồn tại khi cài zram-generator; bật nó là bật hư"
        );
    }

    #[test]
    fn bluetooth_without_hardware_is_rejected() {
        let sel = ServiceSelection {
            bluetooth: true,
            ..Default::default()
        };
        assert!(validate(&sel, false).is_err());
        assert!(validate(&sel, true).is_ok());
    }

    #[test]
    fn audio_stack_uses_lowercase_on_the_wire() {
        let sel = ServiceSelection::default();
        let json = serde_json::to_string(&sel).unwrap();
        assert!(json.contains("\"pipewire\""), "{json}");
        let back: ServiceSelection = serde_json::from_str(&json).unwrap();
        assert_eq!(back.audio, AudioStack::PipeWire);
    }

    #[test]
    fn zram_suggestion_scales_with_ram() {
        let mb = ServiceSelection::suggest_zram_mb(8 * 1024 * 1024 * 1024);
        assert_eq!(mb, 4096);
        let small = ServiceSelection::suggest_zram_mb(512 * 1024 * 1024);
        assert_eq!(small, 256);
    }
}
