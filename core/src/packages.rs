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
}
