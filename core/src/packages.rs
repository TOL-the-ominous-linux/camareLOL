//! Package selection model and AUR policy.
//!
//! The authoritative package *names* live in the Java catalog (which can be
//! updated independently); Rust only stores the selection and the policy rules.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageSelection {
    /// Core system packages, always installed.
    pub core: Vec<String>,
    pub utilities: Vec<String>,
    pub drivers_firmware: Vec<String>,
    pub user_selected: Vec<String>,
    pub aur_packages: Vec<String>,
    /// AUR may be disabled entirely; then `aur_packages` must be ignored.
    pub aur_enabled: bool,
    /// The unprivileged account AUR packages are built as.
    ///
    /// Never `root` and never guessed: when camareLOL itself runs as root (which
    /// it must, to write a disk) this is the *only* thing that decides whose
    /// privileges run a PKGBUILD. Absent means "refuse", not "pick one for me".
    ///
    /// `#[serde(default)]` so a profile saved before this field existed still
    /// loads — a schema bump for an optional string would invalidate every
    /// saved configuration for no safety gain.
    #[serde(default)]
    pub aur_build_user: Option<String>,
}

impl Default for PackageSelection {
    fn default() -> Self {
        PackageSelection {
            core: vec![
                "base".into(),
                "linux".into(),
                "linux-firmware".into(),
                "base-devel".into(),
                "sudo".into(),
                "networkmanager".into(),
                "vim".into(),
                "man-db".into(),
                "man-pages".into(),
            ],
            utilities: vec![],
            drivers_firmware: vec![],
            user_selected: vec![],
            aur_packages: vec![],
            aur_enabled: false,
            aur_build_user: None,
        }
    }
}

impl PackageSelection {
    /// Everything that pacstrap will install, in a stable order.
    pub fn pacstrap_list(&self) -> Vec<String> {
        let mut all: Vec<String> = Vec::new();
        all.extend(self.core.iter().cloned());
        all.extend(self.drivers_firmware.iter().cloned());
        all.extend(self.utilities.iter().cloned());
        all.extend(self.user_selected.iter().cloned());
        all.sort();
        all.dedup();
        all
    }

    /// AUR packages must only be installed when the user enabled AUR.
    pub fn aur_list(&self) -> Vec<String> {
        if self.aur_enabled {
            let mut v = self.aur_packages.clone();
            v.sort();
            v.dedup();
            v
        } else {
            Vec::new()
        }
    }
}

/// Warning the user must see before AUR is used.
pub const AUR_WARNING: &str = "AUR packages are user-submitted build scripts. They are NOT vetted by Arch Linux. camareLOL builds them as a normal user (never as root) and only installs the ones you explicitly selected.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pacstrap_list_is_deduped_and_sorted() {
        let mut sel = PackageSelection::default();
        sel.core.push("git".into());
        sel.utilities.push("git".into());
        sel.user_selected.push("htop".into());

        let list = sel.pacstrap_list();
        assert_eq!(list.iter().filter(|p| *p == "git").count(), 1);
        assert!(list.contains(&"htop".to_string()));
    }

    #[test]
    fn aur_is_ignored_when_disabled() {
        let mut sel = PackageSelection {
            aur_enabled: true,
            ..Default::default()
        };
        sel.aur_packages.push("some-helper".into());
        assert_eq!(sel.aur_list(), vec!["some-helper".to_string()]);
        sel.aur_enabled = false;
        assert!(sel.aur_list().is_empty());
    }

    #[test]
    fn a_profile_saved_before_the_build_user_existed_still_loads() {
        // `aur_build_user` was added to the format; an old file must still
        // parse, or every saved profile on every existing machine breaks.
        let text = r#"
schema_version = 1
language = "en"
system_locale = "en_US.UTF-8"
keymap = "us"
timezone = "UTC"
firmware = "auto"
filesystem = "ext4"
separate_boot = false
separate_home = false
bootloader = "grub"
desktop = "console"
display_server = "auto"
hostname = "archlinux"
root_login_allowed = true
users = []
admin_users = []

[packages]
core = ["base"]
utilities = []
drivers_firmware = []
user_selected = []
aur_packages = ["some-pkg"]
aur_enabled = true

[services]
network_manager = true
bluetooth = false
audio = "pipewire"
printing = false
ssh = false
zram = true
"#;
        let cfg: crate::config::InstallationConfig = toml::from_str(text).unwrap();
        assert!(cfg.packages.aur_enabled, "the fixture really has AUR on");
        assert_eq!(cfg.packages.aur_packages, vec!["some-pkg".to_string()]);
        assert!(
            cfg.packages.aur_build_user.is_none(),
            "a profile with no build user must not invent one"
        );
        // And the consequence: with AUR on, no build user and no claim about
        // who it is, the run is refused rather than built as root.
        assert_eq!(
            crate::aur::plan(&cfg.packages, None, Some("runuser"), None, true)
                .unwrap_err()
                .code,
            crate::error::ErrorCode::AurBuildRefused
        );
    }

    #[test]
    fn the_build_user_survives_a_round_trip() {
        let sel = PackageSelection {
            aur_build_user: Some("builder".into()),
            ..Default::default()
        };
        let text = toml::to_string(&sel).unwrap();
        let back: PackageSelection = toml::from_str(&text).unwrap();
        assert_eq!(back.aur_build_user.as_deref(), Some("builder"));
    }
}
