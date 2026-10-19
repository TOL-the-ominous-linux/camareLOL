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

/// What [`InstallationConfig::load_profile`] found.
///
/// A load step needs to tell these apart: a blank file means "nothing was
/// saved here, start fresh", a file with a *wrong* schema means "refuse, this
/// was written by another version".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileLoad {
    /// A valid profile; these are its choices.
    Restored(Box<InstallationConfig>),
    /// The file exists but holds no configuration at all.
    Empty,
}

impl ProfileLoad {
    pub fn restored(&self) -> Option<&InstallationConfig> {
        match self {
            ProfileLoad::Restored(cfg) => Some(cfg.as_ref()),
            ProfileLoad::Empty => None,
        }
    }

    /// True when there is nothing to apply and the caller should keep its
    /// defaults.
    pub fn is_empty(&self) -> bool {
        matches!(self, ProfileLoad::Empty)
    }
}

/// Reference to a disk chosen earlier. The path is only a hint: before any write
/// the disk is re-scanned and `identity` is compared.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiskReference {
    pub path: String,
    pub identity: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
        match Self::load_profile(path)? {
            ProfileLoad::Restored(cfg) => Ok(*cfg),
            ProfileLoad::Empty => Err(CamarelolError::new(
                ErrorCode::InvalidConfig,
                format!("{path} contains no configuration"),
            )
            .with_remediation(
                "an empty file cannot be installed from — pick a profile that has choices in it",
            )),
        }
    }

    /// Read a profile, telling "nothing saved here" apart from "this file is
    /// not a camareLOL configuration".
    ///
    /// The two answers need different behaviour and collapsing them into one
    /// `Err` is what makes a load step frustrating: an empty file is what you
    /// get from `touch profile.toml`, and it should start the wizard fresh
    /// rather than look like corruption. Anything that *is* in the file and
    /// does not make sense — a schema from another version, a locale that is
    /// not a locale, an admin who is not a user — is refused with a message, and
    /// never quietly replaced by the default. A load step that silently
    /// discards a profile and installs the defaults is how someone ends up with
    /// an empty disk and the wrong keyboard layout.
    pub fn load_profile(path: &str) -> Result<ProfileLoad> {
        let raw = std::fs::read_to_string(path).map_err(|e| {
            CamarelolError::new(ErrorCode::InvalidConfig, format!("cannot read {path}"))
                .with_context(e.to_string())
                .with_remediation("check the path; a saved profile looks like /root/camarelol.toml")
        })?;
        // Whitespace and comments only: a file that exists but carries no
        // choices is not a broken profile, it is a blank one.
        let meaningful = raw
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty() && !l.starts_with('#'));
        if meaningful.is_none() {
            return Ok(ProfileLoad::Empty);
        }
        let mut cfg: InstallationConfig = toml::from_str(&raw).map_err(|e| {
            CamarelolError::new(
                ErrorCode::InvalidConfig,
                format!("{path} is not a valid camareLOL configuration: {e}"),
            )
            .with_remediation("delete it and save a new profile from camareLOL")
        })?;
        cfg.migrate()?;
        cfg.validate()?;
        Ok(ProfileLoad::Restored(Box::new(cfg)))
    }

    /// Write the profile to `path` only if the file is not already there.
    ///
    /// The interactive load step uses this to remember a profile it has just
    /// shown the user. Overwriting is never done here: a profile on disk is
    /// something the user wrote deliberately, and the installer has no business
    /// replacing it with what the wizard happened to be showing.
    pub fn save_if_absent(&self, path: &str) -> Result<bool> {
        if std::path::Path::new(path).exists() {
            return Ok(false);
        }
        self.save(path)?;
        Ok(true)
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
                format!(
                    "configuration schema {other} is not supported (this camareLOL reads version {SCHEMA_VERSION})"
                ),
            )
            .with_remediation("this configuration was written by another camareLOL version")),
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

    // ------------------------------------------------------- the load step

    /// A scratch path of this test's own, so the tests below never share a file
    /// with each other or with a real profile.
    fn scratch(name: &str) -> String {
        static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        // A per-process counter alone reuses yesterday's filenames, and
        // `save_if_absent` would then rightly refuse to write. The timestamp
        // keeps a re-run of the suite honest.
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join("camarelol-config-load-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(format!("{name}-{stamp}-{n}.toml"));
        p.to_str().unwrap().to_string()
    }

    fn full_profile() -> InstallationConfig {
        let mut cfg = sample();
        cfg.keymap = "vn".into();
        cfg.filesystem = "btrfs".into();
        cfg.bootloader = "systemd-boot".into();
        cfg.desktop = "sway".into();
        cfg.display_server = "wayland".into();
        cfg.separate_home = true;
        cfg.separate_boot = true;
        cfg.swap_bytes = Some(2 * 1024 * 1024 * 1024);
        cfg.hostname = "laptop".into();
        cfg.users = vec!["alice".into()];
        cfg.admin_users = vec!["alice".into()];
        cfg.root_login_allowed = false;
        cfg.target_disk = Some(DiskReference {
            path: "/dev/nvme0n1".into(),
            identity: "WD-1234".into(),
            size_bytes: 512_000_000_000,
        });
        cfg.packages.user_selected = vec!["firefox".into(), "htop".into()];
        cfg.packages.aur_enabled = true;
        cfg.packages.aur_packages = vec!["yay".into()];
        cfg.packages.aur_build_user = Some("builder".into());
        cfg.services.bluetooth = true;
        cfg.services.ssh = true;
        cfg.services.audio = crate::services::AudioStack::PulseAudio;
        cfg
    }

    /// Every choice the wizard stores has to come back, or "save and resume" is
    /// a feature that exists.
    #[test]
    fn a_saved_profile_survives_save_load_save() {
        let original = full_profile();
        let path = scratch("full");
        original.save(&path).unwrap();

        let loaded = InstallationConfig::load(&path).unwrap();

        assert_eq!(loaded.language, original.language);
        assert_eq!(loaded.system_locale, original.system_locale);
        assert_eq!(loaded.keymap, original.keymap);
        assert_eq!(loaded.timezone, original.timezone);
        assert_eq!(loaded.filesystem, original.filesystem);
        assert_eq!(loaded.separate_home, original.separate_home);
        assert_eq!(loaded.separate_boot, original.separate_boot);
        assert_eq!(loaded.swap_bytes, original.swap_bytes);
        assert_eq!(loaded.bootloader, original.bootloader);
        assert_eq!(loaded.desktop, original.desktop);
        assert_eq!(loaded.display_server, original.display_server);
        assert_eq!(loaded.hostname, original.hostname);
        assert_eq!(loaded.users, original.users);
        assert_eq!(loaded.admin_users, original.admin_users);
        assert!(!loaded.root_login_allowed);
        assert_eq!(loaded.target_disk, original.target_disk);
        assert_eq!(
            loaded.packages.user_selected,
            original.packages.user_selected
        );
        assert!(loaded.packages.aur_enabled);
        assert_eq!(loaded.packages.aur_packages, original.packages.aur_packages);
        assert_eq!(
            loaded.packages.aur_build_user,
            Some("builder".to_string()),
            "the AUR build user is a choice and must be resumed"
        );
        assert!(loaded.services.bluetooth);
        assert!(loaded.services.ssh);
        assert_eq!(
            loaded.services.audio,
            crate::services::AudioStack::PulseAudio
        );

        // And a second round trip is a fixed point, not a slow drift.
        let again = scratch("full-again");
        loaded.save(&again).unwrap();
        let twice = InstallationConfig::load(&again).unwrap();
        assert_eq!(
            twice.to_toml().unwrap(),
            loaded.to_toml().unwrap(),
            "a second save must be byte-identical"
        );
    }

    #[test]
    fn loading_a_corrupt_file_is_refused_and_says_so() {
        let path = scratch("corrupt");
        std::fs::write(&path, "schema_version = 1\ndesktop = [[[\n").unwrap();
        let err = InstallationConfig::load_profile(&path).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidConfig);
        assert!(
            err.message.contains(&path) || err.message.contains("configuration"),
            "{}",
            err.message
        );
        assert!(
            err.remediation.is_some(),
            "a refused load must tell the user what to do about it"
        );
    }

    /// A profile from another version of camareLOL is refused, not reinterpreted.
    #[test]
    fn a_wrong_schema_version_is_refused_rather_than_guessed_at() {
        let path = scratch("schema-99");
        let mut text = full_profile().to_toml().unwrap();
        text = text.replace("schema_version = 1", "schema_version = 99");
        std::fs::write(&path, text).unwrap();

        let err = InstallationConfig::load_profile(&path).unwrap_err();
        assert_eq!(err.code, ErrorCode::UnsupportedSchema);
        assert!(err.message.contains("99"), "{}", err.message);
        assert!(err.message.contains("1"), "it must name what is expected");
    }

    #[test]
    fn an_empty_file_starts_fresh_instead_of_failing() {
        // `touch profile.toml` is what a user does when they mean "save here".
        for content in ["", "   \n\n", "# only a comment\n"] {
            let path = scratch("empty");
            std::fs::write(&path, content).unwrap();

            match InstallationConfig::load_profile(&path).unwrap() {
                ProfileLoad::Empty => {}
                ProfileLoad::Restored(_) => {
                    panic!("{content:?} carries no choices and must not produce a profile")
                }
            }
            // The strict entry point still refuses it, because an install from
            // an empty file would silently use the defaults.
            assert_eq!(
                InstallationConfig::load(&path).unwrap_err().code,
                ErrorCode::InvalidConfig
            );
        }
    }

    #[test]
    fn a_truncated_profile_is_refused_rather_than_half_applied() {
        // The dangerous half of this is "apply what parsed, default the rest":
        // a missing `target_disk` would silently install to whatever the wizard
        // happens to point at.
        let path = scratch("truncated");
        std::fs::write(
            &path,
            "schema_version = 1\nlanguage = \"vi\"\nsystem_locale = \"vi_VN.UTF-8\"\n",
        )
        .unwrap();
        let err = InstallationConfig::load_profile(&path).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidConfig);
        assert!(
            err.message.contains("missing field"),
            "the error should say what is missing: {}",
            err.message
        );
    }

    #[test]
    fn an_admin_who_is_not_a_user_is_refused_on_load() {
        let mut cfg = full_profile();
        cfg.admin_users = vec!["ghost".into()];
        let path = scratch("ghost-admin");
        cfg.save(&path).unwrap();
        let err = InstallationConfig::load_profile(&path).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidAccount);
    }

    // ------------------------------------------------------- passwords

    /// The guarantee, stated three ways, because it is the one that must not
    /// quietly regress: a password is never in a file, never comes back out of
    /// one, and is not even a field the format knows how to read.
    #[test]
    fn passwords_are_never_written_and_never_come_back() {
        let mut cfg = full_profile();
        cfg.set_passwords(Some("root-secret-42"), &[("alice", "alice-secret-42")]);
        assert!(cfg.root_password().is_some(), "the fixture really has one");

        let path = scratch("with-passwords");
        cfg.save(&path).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();

        for secret in ["root-secret-42", "alice-secret-42"] {
            assert!(!text.contains(secret), "a password reached the file");
        }
        assert!(
            !text.to_ascii_lowercase().contains("password"),
            "the field must not even be named: {text}"
        );
        assert!(!text.contains("[passwords]"), "{text}");

        let loaded = InstallationConfig::load(&path).unwrap();
        assert!(
            loaded.passwords.is_empty(),
            "loading must not restore a password from anywhere"
        );
        assert!(loaded.root_password().is_none());
        assert!(loaded.user_password("alice").is_none());
        assert!(
            loaded.account_plan(None).users[0].password.is_none(),
            "the account plan must not carry one either"
        );
    }

    /// Even a hand-edited file that *does* contain the field cannot get a
    /// password back into the running installer.
    #[test]
    fn a_file_that_spell_out_a_password_still_cannot_restore_one() {
        let path = scratch("hand-edited-password");
        let mut text = full_profile().to_toml().unwrap();
        text.push_str("\n[passwords]\nroot = \"written-by-hand\"\n");
        std::fs::write(&path, text).unwrap();

        let loaded = InstallationConfig::load(&path).unwrap();
        assert!(
            loaded.passwords.is_empty(),
            "`#[serde(skip)]` has to mean the format cannot read them back"
        );
        assert!(loaded.root_password().is_none());
        assert!(!loaded.to_toml().unwrap().contains("written-by-hand"));
    }

    #[test]
    fn save_if_absent_never_overwrites_a_profile() {
        let path = scratch("keep-mine");
        let mine = InstallationConfig {
            hostname: "mine".into(),
            ..Default::default()
        };
        mine.save(&path).unwrap();

        let other = InstallationConfig {
            hostname: "theirs".into(),
            ..Default::default()
        };
        assert!(!other.save_if_absent(&path).unwrap(), "must not overwrite");
        assert_eq!(InstallationConfig::load(&path).unwrap().hostname, "mine");

        let fresh = scratch("no-profile-yet");
        assert!(other.save_if_absent(&fresh).unwrap());
        assert_eq!(InstallationConfig::load(&fresh).unwrap().hostname, "theirs");
        assert!(!other.save_if_absent(&fresh).unwrap());
    }

    #[test]
    fn loading_a_file_that_is_not_there_says_so_rather_than_defaulting() {
        let err = InstallationConfig::load_profile("/nonexistent/camarelol/nope.toml").unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidConfig);
        assert!(err.message.contains("cannot read"), "{}", err.message);
    }
}
