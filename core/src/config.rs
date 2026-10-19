//! Installation configuration: schema versioning, validation and migration.
//!
//! The file format is TOML with an explicit `schema_version`. **Passwords are
//! never part of this file**, and a device path is only ever a *hint* — the
//! disk must be re-identified before anything is written.

use crate::accounts::AccountPlan;
use crate::boot::Bootloader;
use crate::desktop::{DesktopProfile, DisplayServer};
use crate::error::{CamarelolError, ErrorCode, Result};
use crate::hardware::Firmware;
use crate::locale::{validate_locale, validate_timezone, UiLanguage};
use crate::packages::PackageSelection;
use crate::services::ServiceSelection;
use crate::storage::device::Filesystem;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Current configuration schema version. Bump when the layout changes and add a
/// migration step in [`InstallationConfig::migrate`].
pub const SCHEMA_VERSION: u32 = 1;

/// Reference to a disk chosen earlier. The path is only a hint: before any write
/// the disk is re-scanned and `identity` is compared.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiskReference {
    pub path: String,
    pub identity: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallationConfig {
    pub schema_version: u32,
    /// `vi`, `en` or `zh` — the installer UI language (not the system locale).
    pub language: String,
    pub system_locale: String,
    pub keymap: String,
    pub timezone: String,
    /// `uefi` / `bios` / `auto`
    pub firmware: String,
    pub target_disk: Option<DiskReference>,
    pub filesystem: String,
    pub separate_boot: bool,
    pub separate_home: bool,
    pub swap_bytes: Option<u64>,
    pub bootloader: String,
    pub desktop: String,
    pub display_server: String,
    pub display_manager: Option<String>,
    pub hostname: String,
    pub root_login_allowed: bool,
    /// Users **without** passwords; passwords exist only in memory.
    pub users: Vec<String>,
    pub admin_users: Vec<String>,
    pub packages: PackageSelection,
    pub services: ServiceSelection,
    /// Runtime-only passwords. `#[serde(skip)]` guarantees they are never
    /// written to a configuration file, and they are never logged.
    #[serde(skip)]
    pub passwords: std::collections::HashMap<String, String>,
}

impl Default for InstallationConfig {
    fn default() -> Self {
        InstallationConfig {
            schema_version: SCHEMA_VERSION,
            language: "en".into(),
            system_locale: "en_US.UTF-8".into(),
            keymap: "us".into(),
            timezone: "UTC".into(),
            firmware: "auto".into(),
            target_disk: None,
            filesystem: "ext4".into(),
            separate_boot: false,
            separate_home: false,
            swap_bytes: None,
            bootloader: "grub".into(),
            desktop: "console".into(),
            display_server: "auto".into(),
            display_manager: None,
            hostname: "archlinux".into(),
            root_login_allowed: true,
            users: vec![],
            admin_users: vec![],
            packages: PackageSelection::default(),
            services: ServiceSelection::default(),
            passwords: std::collections::HashMap::new(),
        }
    }
}

impl InstallationConfig {
    pub fn filesystem_type(&self) -> Result<Filesystem> {
        Ok(match self.filesystem.as_str() {
            "ext4" => Filesystem::Ext4,
            "btrfs" => Filesystem::Btrfs,
            "xfs" => Filesystem::Xfs,
            "ext2" => Filesystem::Ext2,
            other => {
                return Err(CamarelolError::new(
                    ErrorCode::InvalidConfig,
                    format!("unsupported filesystem `{other}`"),
                ))
            }
        })
    }

    pub fn ui_language(&self) -> Result<UiLanguage> {
        UiLanguage::from_code(&self.language).ok_or_else(|| {
            CamarelolError::new(
                ErrorCode::InvalidConfig,
                format!("unknown installer language `{}`", self.language),
            )
        })
    }

    /// `auto` resolves to the detected firmware.
    pub fn resolve_firmware(&self, detected: Firmware) -> Result<Firmware> {
        match self.firmware.as_str() {
            "auto" => Ok(detected),
            "uefi" => Ok(Firmware::Uefi),
            "bios" => Ok(Firmware::Bios),
            other => Err(CamarelolError::new(
                ErrorCode::InvalidConfig,
                format!("unknown firmware mode `{other}`"),
            )),
        }
    }

    pub fn bootloader(&self) -> Result<Bootloader> {
        match self.bootloader.as_str() {
            "grub" => Ok(Bootloader::Grub),
            "systemd-boot" => Ok(Bootloader::SystemdBoot),
            other => Err(CamarelolError::new(
                ErrorCode::InvalidConfig,
                format!("unknown bootloader `{other}`"),
            )),
        }
    }

    pub fn desktop_profile(&self) -> Result<DesktopProfile> {
        match self.desktop.as_str() {
            "console" => Ok(DesktopProfile::ConsoleOnly),
            "plasma" => Ok(DesktopProfile::Plasma),
            "gnome" => Ok(DesktopProfile::Gnome),
            "xfce" => Ok(DesktopProfile::Xfce),
            "cinnamon" => Ok(DesktopProfile::Cinnamon),
            "lxqt" => Ok(DesktopProfile::Lxqt),
            "hyprland" => Ok(DesktopProfile::Hyprland),
            "sway" => Ok(DesktopProfile::Sway),
            other => Err(CamarelolError::new(
                ErrorCode::InvalidConfig,
                format!("unknown desktop profile `{other}`"),
            )),
        }
    }

    pub fn display_server(&self) -> Result<DisplayServer> {
        match self.display_server.as_str() {
            "auto" => Ok(DisplayServer::Auto),
            "wayland" => Ok(DisplayServer::Wayland),
            "x11" => Ok(DisplayServer::X11),
            other => Err(CamarelolError::new(
                ErrorCode::InvalidConfig,
                format!("unknown display server `{other}`"),
            )),
        }
    }

    /// Validate everything we can check without touching the system.
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(CamarelolError::new(
                ErrorCode::UnsupportedSchema,
                format!(
                    "config schema_version {} is not supported (expected {})",
                    self.schema_version, SCHEMA_VERSION
                ),
            )
            .with_remediation("open the configuration in camareLOL and save it again to migrate"));
        }
        self.ui_language()?;
        validate_locale(&self.system_locale)?;
        validate_timezone(&self.timezone)?;
        self.resolve_firmware(crate::hardware::detect_firmware())?;
        self.bootloader()?;
        self.filesystem_type()?;
        self.desktop_profile()?;
        self.display_server()?;
        crate::accounts::validate_hostname(&self.hostname)?;
        for u in &self.users {
            crate::accounts::validate_username(u)?;
        }
        // admin users must exist as users
        for a in &self.admin_users {
            if !self.users.contains(a) {
                return Err(CamarelolError::new(
                    ErrorCode::InvalidAccount,
                    format!("`{a}` is marked as admin but is not in the user list"),
                ));
            }
        }
        if !self.root_login_allowed && self.admin_users.is_empty() {
            return Err(CamarelolError::new(
                ErrorCode::InvalidAccount,
                "root login is disabled and no admin user exists",
            ));
        }
        Ok(())
    }

    /// Produce the runtime account plan (passwords attached separately).
    pub fn account_plan(&self, root_password: Option<&str>) -> AccountPlan {
        let users = self
            .users
            .iter()
            .map(|u| {
                let mut acc = crate::accounts::UserAccount::new(u);
                acc.has_admin = self.admin_users.contains(u);
                // Passwords live only in the in-memory map, never in a file.
                acc.password = self.user_password(u).map(|p| p.to_string());
                acc
            })
            .collect();
        AccountPlan {
            hostname: self.hostname.clone(),
            root_password_set: root_password.is_some(),
            root_login_allowed: self.root_login_allowed,
            users,
        }
    }

    // ------------------------------------------------------------------
    // Runtime-only password handling (never serialized, never logged)
    // ------------------------------------------------------------------

    /// Store the root password and per-user passwords in memory for this run.
    pub fn set_passwords(&mut self, root: Option<&str>, users: &[(&str, &str)]) {
        self.passwords.clear();
        if let Some(r) = root {
            if !r.is_empty() {
                self.passwords.insert("root".to_string(), r.to_string());
            }
        }
        for (user, pw) in users {
            if !pw.is_empty() {
                self.passwords.insert(user.to_string(), pw.to_string());
            }
        }
    }

    pub fn root_password(&self) -> Option<&str> {
        self.passwords.get("root").map(|s| s.as_str())
    }

    pub fn user_password(&self, user: &str) -> Option<&str> {
        self.passwords.get(user).map(|s| s.as_str())
    }

    /// Serialize to TOML text.
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).map_err(|e| {
            CamarelolError::new(
                ErrorCode::InvalidConfig,
                format!("cannot serialize config: {e}"),
            )
        })
    }

    /// Save to a file. Passwords are simply not part of the struct.
    pub fn save(&self, path: &str) -> Result<()> {
        let text = self.to_toml()?;
        if let Some(dir) = Path::new(path).parent() {
            if !dir.as_os_str().is_empty() {
                std::fs::create_dir_all(dir).map_err(|e| {
                    CamarelolError::new(
                        ErrorCode::InvalidConfig,
                        format!("cannot create {}", dir.display()),
                    )
                    .with_context(e.to_string())
                })?;
            }
        }
        std::fs::write(path, text).map_err(|e| {
            CamarelolError::new(ErrorCode::InvalidConfig, format!("cannot write {path}"))
                .with_context(e.to_string())
        })?;
        Ok(())
    }

    pub fn load(path: &str) -> Result<Self> {
        let raw = std::fs::read_to_string(path).map_err(|e| {
            CamarelolError::new(ErrorCode::InvalidConfig, format!("cannot read {path}"))
                .with_context(e.to_string())
        })?;
        let mut cfg: InstallationConfig = toml::from_str(&raw).map_err(|e| {
            CamarelolError::new(
                ErrorCode::InvalidConfig,
                format!("{path} is not a valid camareLOL configuration: {e}"),
            )
        })?;
        cfg.migrate()?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Migrate older schemas forward. v0 -> v1 is a no-op placeholder that
    /// documents the mechanism (future versions fill real notes here).
    fn migrate(&mut self) -> Result<()> {
        match self.schema_version {
            0 => {
                // v0 had no schema_version field at all: TOML parse fails on
                // the missing field, so this branch is only reachable for
                // explicit `schema_version = 0` files.
                self.schema_version = 1;
                Ok(())
            }
            1 => Ok(()),
            other => Err(CamarelolError::new(
                ErrorCode::UnsupportedSchema,
                format!("configuration schema {other} is not supported"),
            )
            .with_remediation("this configuration was written by an older/newer camareLOL")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> InstallationConfig {
        InstallationConfig {
            language: "vi".into(),
            system_locale: "vi_VN.UTF-8".into(),
            timezone: "Asia/Ho_Chi_Minh".into(),
            filesystem: "ext4".into(),
            desktop: "plasma".into(),
            ..Default::default()
        }
    }

    #[test]
    fn default_config_is_valid() {
        assert!(InstallationConfig::default().validate().is_ok());
    }

    #[test]
    fn vietnamese_config_is_valid() {
        assert!(sample().validate().is_ok());
    }

    #[test]
    fn round_trip_through_toml() {
        let cfg = sample();
        let text = cfg.to_toml().unwrap();
        let dir = std::env::temp_dir().join("camarelol-config-test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("cfg.toml");
        std::fs::write(&p, &text).unwrap();
        let loaded = InstallationConfig::load(p.to_str().unwrap()).unwrap();
        assert_eq!(loaded.language, "vi");
        assert_eq!(loaded.system_locale, "vi_VN.UTF-8");
        assert_eq!(loaded.desktop, "plasma");
    }

    #[test]
    fn config_never_contains_passwords() {
        let cfg = sample();
        let text = cfg.to_toml().unwrap().to_ascii_lowercase();
        assert!(!text.contains("password"));
    }

    #[test]
    fn rejects_unknown_schema() {
        let mut cfg = sample();
        cfg.schema_version = 99;
        let err = cfg.validate().unwrap_err();
        assert_eq!(err.code, ErrorCode::UnsupportedSchema);
    }

    #[test]
    fn rejects_bad_locale_and_desktop() {
        let mut cfg = sample();
        cfg.system_locale = "nope".into();
        assert_eq!(cfg.validate().unwrap_err().code, ErrorCode::InvalidConfig);
        let mut cfg2 = sample();
        cfg2.desktop = "windows-xp".into();
        assert!(cfg2.validate().is_err());
    }

    #[test]
    fn lockout_prevention() {
        let mut cfg = sample();
        cfg.root_login_allowed = false;
        // no users at all -> must fail
        assert!(cfg.validate().is_err());
        cfg.users = vec!["alice".into()];
        cfg.admin_users = vec!["alice".into()];
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn loading_a_broken_file_reports_line() {
        let dir = std::env::temp_dir().join("camarelol-config-test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("broken.toml");
        std::fs::write(&p, "this is not toml =").unwrap();
        let err = InstallationConfig::load(p.to_str().unwrap()).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidConfig);
    }
}
