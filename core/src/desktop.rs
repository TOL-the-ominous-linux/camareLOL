//! Desktop / window-manager profiles.
//!
//! Profiles declare their packages, display manager, display server support and
//! known conflicts. camareLOL refuses combinations it cannot satisfy instead of
//! silently installing something else.

use crate::error::{CamarelolError, ErrorCode, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DisplayServer {
    Wayland,
    X11,
    Auto,
}

impl DisplayServer {
    /// All options, in display order.
    pub const ALL: [DisplayServer; 3] = [DisplayServer::Auto, DisplayServer::Wayland, DisplayServer::X11];

    pub fn as_str(&self) -> &'static str {
        match self {
            DisplayServer::Wayland => "Wayland",
            DisplayServer::X11 => "X11",
            DisplayServer::Auto => "automatic",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DesktopProfile {
    Plasma,
    Gnome,
    Xfce,
    Cinnamon,
    Lxqt,
    Hyprland,
    Sway,
    ConsoleOnly,
}

impl DesktopProfile {
    /// All profiles, in display order.
    pub const ALL: [DesktopProfile; 8] = [
        DesktopProfile::Plasma,
        DesktopProfile::Gnome,
        DesktopProfile::Xfce,
        DesktopProfile::Cinnamon,
        DesktopProfile::Lxqt,
        DesktopProfile::Hyprland,
        DesktopProfile::Sway,
        DesktopProfile::ConsoleOnly,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            DesktopProfile::Plasma => "KDE Plasma",
            DesktopProfile::Gnome => "GNOME",
            DesktopProfile::Xfce => "Xfce",
            DesktopProfile::Cinnamon => "Cinnamon",
            DesktopProfile::Lxqt => "LXQt",
            DesktopProfile::Hyprland => "Hyprland",
            DesktopProfile::Sway => "Sway",
            DesktopProfile::ConsoleOnly => "No desktop (console only)",
        }
    }

    pub fn packages(&self) -> Vec<String> {
        match self {
            DesktopProfile::Plasma => vec!["plasma".into(), "konsole".into(), "dolphin".into(), "sddm".into()],
            DesktopProfile::Gnome => vec!["gnome".into(), "gnome-tweaks".into(), "gdm".into()],
            DesktopProfile::Xfce => vec!["xfce4".into(), "xfce4-goodies".into(), "lightdm".into(), "lightdm-gtk-greeter".into()],
            DesktopProfile::Cinnamon => vec!["cinnamon".into(), "lightdm".into(), "lightdm-gtk-greeter".into()],
            DesktopProfile::Lxqt => vec!["lxqt".into(), "sddm".into()],
            DesktopProfile::Hyprland => vec!["hyprland".into(), "kitty".into(), "wofi".into(), "waybar".into(), "greetd".into(), "greetd-tuigreeter".into()],
            DesktopProfile::Sway => vec!["sway".into(), "foot".into(), "wofi".into(), "waybar".into(), "greetd".into(), "greetd-tuigreeter".into()],
            DesktopProfile::ConsoleOnly => vec![],
        }
    }

    /// Display servers this profile can realistically use.
    pub fn display_servers(&self) -> Vec<DisplayServer> {
        match self {
            DesktopProfile::Hyprland | DesktopProfile::Sway => vec![DisplayServer::Wayland],
            DesktopProfile::Plasma
            | DesktopProfile::Gnome
            | DesktopProfile::Xfce
            | DesktopProfile::Cinnamon
            | DesktopProfile::Lxqt => vec![DisplayServer::Wayland, DisplayServer::X11],
            DesktopProfile::ConsoleOnly => vec![],
        }
    }

    /// Display managers this profile is known to work with.
    pub fn display_managers(&self) -> Vec<&'static str> {
        match self {
            DesktopProfile::Plasma | DesktopProfile::Lxqt => vec!["sddm"],
            DesktopProfile::Gnome => vec!["gdm"],
            DesktopProfile::Xfce | DesktopProfile::Cinnamon => vec!["lightdm"],
            DesktopProfile::Hyprland | DesktopProfile::Sway => vec!["greetd"],
            DesktopProfile::ConsoleOnly => vec![],
        }
    }

    pub fn supports_wayland(&self) -> bool {
        self.display_servers().contains(&DisplayServer::Wayland)
    }

    /// Extra system services implied by this profile.
    pub fn services(&self) -> Vec<&'static str> {
        match self {
            DesktopProfile::ConsoleOnly => vec![],
            _ => vec!["NetworkManager"],
        }
    }
}

/// Resolve `Auto` to an actual display server.
pub fn resolve_display_server(profile: DesktopProfile, requested: DisplayServer) -> Result<DisplayServer> {
    match requested {
        DisplayServer::Auto => {
            if profile.supports_wayland() {
                Ok(DisplayServer::Wayland)
            } else if profile.display_servers().contains(&DisplayServer::X11) {
                Ok(DisplayServer::X11)
            } else {
                // Console-only: there is nothing to resolve.
                Ok(DisplayServer::Auto)
            }
        }
        other => {
            if !profile.display_servers().contains(&other) {
                return Err(CamarelolError::new(
                    ErrorCode::InvalidConfig,
                    format!("{} cannot run on {}", profile.as_str(), other.as_str()),
                )
                .with_remediation(format!("choose one of: {:?}", profile.display_servers())));
            }
            Ok(other)
        }
    }
}

/// Cross-check profile + display manager + display server.
pub fn validate_desktop_config(
    profile: DesktopProfile,
    server: DisplayServer,
    display_manager: Option<&str>,
) -> Result<()> {
    resolve_display_server(profile, server)?;
    if profile == DesktopProfile::ConsoleOnly {
        if display_manager.is_some() {
            return Err(CamarelolError::new(
                ErrorCode::InvalidConfig,
                "a console-only install cannot use a display manager",
            ));
        }
        return Ok(());
    }
    if let Some(dm) = display_manager {
        if !profile.display_managers().contains(&dm) {
            return Err(CamarelolError::new(
                ErrorCode::InvalidConfig,
                format!("{} is not a known display manager for {}", dm, profile.as_str()),
            )
            .with_remediation(format!("use one of: {:?}", profile.display_managers())));
        }
    }
    Ok(())
}

/// Convenience re-export used by the install orchestrator.
pub fn packages_for(profile: DesktopProfile) -> Vec<String> {
    profile.packages()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sway_is_wayland_only() {
        assert!(resolve_display_server(DesktopProfile::Sway, DisplayServer::X11).is_err());
        assert_eq!(
            resolve_display_server(DesktopProfile::Sway, DisplayServer::Auto).unwrap(),
            DisplayServer::Wayland
        );
    }

    #[test]
    fn plasma_accepts_both() {
        assert_eq!(
            resolve_display_server(DesktopProfile::Plasma, DisplayServer::X11).unwrap(),
            DisplayServer::X11
        );
    }

    #[test]
    fn console_only_rejects_display_manager() {
        assert!(validate_desktop_config(DesktopProfile::ConsoleOnly, DisplayServer::Auto, Some("sddm")).is_err());
        assert!(validate_desktop_config(DesktopProfile::ConsoleOnly, DisplayServer::Auto, None).is_ok());
    }

    #[test]
    fn gnome_uses_gdm() {
        assert!(validate_desktop_config(DesktopProfile::Gnome, DisplayServer::Wayland, Some("gdm")).is_ok());
        assert!(validate_desktop_config(DesktopProfile::Gnome, DisplayServer::Wayland, Some("sddm")).is_err());
    }
}
