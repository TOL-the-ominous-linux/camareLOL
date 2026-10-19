//! Package selection model, catalog and AUR policy.
//!
//! # Where the names come from
//!
//! The names in [`CATALOG`] are copied from the places that already agreed on
//! them: the Java `PackageCatalog` (`GROUPS`), the Python `package_recommend`
//! groups, and the lists the installer itself builds
//! ([`PackageSelection::default`], `ServiceSelection::packages`,
//! `DesktopProfile::packages`).
//!
//! **A name the catalog does not have is a name the search will not offer.**
//! That is the point of having one: `greetd-tuigreeter` and `p7zip` both passed
//! every unit test in this project and both failed inside a VM, at `pacstrap`.
//! The catalog is the one place a name has to appear to become findable, and
//! `every_installable_package_is_in_the_catalog` fails when somebody adds one
//! to an install path and forgets it here.

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

// ---------------------------------------------------------------------------
// The catalog
// ---------------------------------------------------------------------------

/// Where a package comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageSource {
    /// An official Arch repository.
    Official,
    /// The Arch User Repository: user-submitted PKGBUILDs, not vetted.
    Aur,
}

impl PackageSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            PackageSource::Official => "extra",
            PackageSource::Aur => "AUR",
        }
    }
}

/// One searchable entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackageEntry {
    pub name: &'static str,
    pub description: &'static str,
    pub source: PackageSource,
    /// When non-empty, the package is only offered for these desktop profiles.
    ///
    /// `xdg-desktop-portal` is the case that matters: it belongs on GNOME and
    /// KDE, is a dependency of packages a Sway install already pulls in, and
    /// offering it to a console-only profile would put a package in `pacstrap`
    /// for nothing. Empty means "any profile".
    pub profiles: &'static [&'static str],
}

const fn official(name: &'static str, description: &'static str) -> PackageEntry {
    PackageEntry {
        name,
        description,
        source: PackageSource::Official,
        profiles: &[],
    }
}

const fn official_for(
    name: &'static str,
    description: &'static str,
    profiles: &'static [&'static str],
) -> PackageEntry {
    PackageEntry {
        name,
        description,
        source: PackageSource::Official,
        profiles,
    }
}

const fn aur(name: &'static str, description: &'static str) -> PackageEntry {
    PackageEntry {
        name,
        description,
        source: PackageSource::Aur,
        profiles: &[],
    }
}

/// Every package camareLOL is willing to name.
///
/// Names verified against the Arch package database and against the other two
/// catalogs in this repository (Java, Python). A name that cannot be found here
/// is never offered to the user and never typed into a selection by the search.
pub const CATALOG: &[PackageEntry] = &[
    // --- the base system, from PackageSelection::default ---
    official("base", "minimal package set"),
    official("linux", "the kernel"),
    official("linux-firmware", "firmware blobs"),
    official("base-devel", "compilers and build tools"),
    official("sudo", "privilege escalation"),
    official("networkmanager", "network management"),
    official("vim", "text editor"),
    official("man-db", "man pages on your machine"),
    official("man-pages", "manual pages for the Arch packages"),
    // --- utilities, from the Java catalog's `utilities` group ---
    official("htop", "process viewer"),
    official("btop", "resource monitor"),
    official("git", "version control"),
    official("firefox", "web browser"),
    // `p7zip` left the Arch repositories; `7zip` is the package that exists.
    official("7zip", "archiving"),
    official("unzip", "unpack zip archives"),
    official("reflector", "mirror ranking tool"),
    // --- fonts and input methods, pulled in by the wizard itself ---
    official("noto-fonts", "Noto fonts, covers Vietnamese"),
    official("noto-fonts-cjk", "Noto CJK fonts, covers Chinese"),
    official("ibus", "input method framework"),
    // --- firmware and drivers ---
    official("mesa", "OpenGL/Vulkan drivers for Intel, AMD and VMware"),
    official("intel-ucode", "Intel CPU microcode"),
    official("amd-ucode", "AMD CPU microcode"),
    official("bluez", "Bluetooth stack"),
    official("bluez-utils", "Bluetooth command line tools"),
    // --- the services a profile can turn on ---
    official("pipewire", "low-latency sound and video server"),
    official("pipewire-pulse", "PulseAudio compatibility for PipeWire"),
    official("pipewire-alsa", "ALSA support for PipeWire"),
    official("wireplumber", "PipeWire session manager"),
    official("pulseaudio", "sound server"),
    official("pulseaudio-alsa", "ALSA support for PulseAudio"),
    official("openssh", "SSH server"),
    official("cups", "printing"),
    official("zram-generator", "compressed RAM swap"),
    // --- desktops, from DesktopProfile::packages ---
    official("plasma", "KDE Plasma desktop"),
    official("konsole", "KDE terminal emulator"),
    official("dolphin", "KDE file manager"),
    official("sddm", "Simple Desktop Display Manager"),
    official("gnome", "GNOME desktop"),
    official("gnome-tweaks", "GNOME settings"),
    official("gdm", "GNOME display manager"),
    official("xfce4", "Xfce desktop"),
    official("xfce4-goodies", "extra Xfce tools"),
    official("lightdm", "LightDM display manager"),
    official("lightdm-gtk-greeter", "GTK greeter for LightDM"),
    official("cinnamon", "Cinnamon desktop"),
    official("lxqt", "LXQt desktop"),
    official("hyprland", "Hyprland Wayland compositor"),
    official("sway", "Sway Wayland compositor"),
    official("kitty", "fast terminal emulator"),
    official("foot", "Wayland terminal emulator"),
    official("wofi", "Wayland launcher"),
    official("waybar", "Wayland status bar"),
    official("greetd", "minimal display manager"),
    // The package is `greetd-tuigreet`. Written `greetd-tuigreeter` it passed
    // every unit test here and failed in the VM, at pacstrap.
    official("greetd-tuigreet", "terminal greeter for greetd"),
    official_for(
        "xdg-desktop-portal",
        "portal used by GNOME and KDE on Wayland",
        &["gnome", "plasma"],
    ),
    // --- AUR: only offered when the user has enabled AUR ---
    aur("yay", "AUR helper"),
    aur("paru", "AUR helper"),
    aur("pikaur", "AUR helper"),
    aur("trizen", "AUR helper"),
    aur("ibus-bamboo", "Vietnamese input method for IBus"),
    aur("fcitx5-bamboo", "Vietnamese input method for Fcitx5"),
];

/// Look a package up by name. Returns `None` for a name camareLOL does not know.
pub fn lookup(name: &str) -> Option<&'static PackageEntry> {
    CATALOG
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(name.trim()))
}

/// True when `name` is a package this installer is willing to name.
pub fn exists(name: &str) -> bool {
    lookup(name).is_some()
}

/// Is this entry offered for `profile`? `None` means "console only".
fn fits_profile(entry: &PackageEntry, profile: Option<&str>) -> bool {
    if entry.profiles.is_empty() {
        return true;
    }
    match profile {
        Some(wanted) => entry.profiles.contains(&wanted),
        // No desktop: the profile installs no portal and needs none.
        None => false,
    }
}

/// Search the catalog.
///
/// * An empty query returns everything the profile and the AUR setting allow —
///   that is what the packages screen shows before anyone types.
/// * Matching is case-insensitive and matches a prefix of the name first, then
///   a substring of the name, then the description. A prefix is ranked first
///   because "fire" should mean `firefox` and not `firefox-esr` on line 4.
/// * AUR entries appear **only** when `aur_enabled`. There is no key that
///   reaches them otherwise, which is what makes "never as root" reachable
///   from the keyboard at all.
///
/// The result is a slice of [`CATALOG`], so a name can never appear here that
/// is not in the catalog.
pub fn search(query: &str, profile: Option<&str>, aur_enabled: bool) -> Vec<&'static PackageEntry> {
    let q = query.trim().to_ascii_lowercase();
    let mut hits: Vec<(u8, &'static PackageEntry)> = Vec::new();
    for entry in CATALOG {
        if entry.source == PackageSource::Aur && !aur_enabled {
            continue;
        }
        if !fits_profile(entry, profile) {
            continue;
        }
        let rank = match entry.name.to_ascii_lowercase().find(&q) {
            Some(0) => 0,
            Some(_) => 1,
            None => {
                if entry.description.to_ascii_lowercase().contains(&q) {
                    2
                } else {
                    continue;
                }
            }
        };
        hits.push((rank, entry));
    }
    // Stable within a rank, so the unfiltered list keeps catalog order and a
    // re-typed query does not shuffle the rows under the cursor.
    hits.sort_by_key(|(rank, _)| *rank);
    hits.into_iter().map(|(_, e)| e).collect()
}

/// Names of the entries the search would offer, for tests and for the TUI.
pub fn searchable_names(query: &str, profile: Option<&str>, aur_enabled: bool) -> Vec<String> {
    search(query, profile, aur_enabled)
        .into_iter()
        .map(|e| e.name.to_string())
        .collect()
}

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

    // ------------------------------------------------------------- catalog

    /// The catalog is the only place a name comes from, so everything the
    /// installer can end up asking pacstrap for must appear in it.
    ///
    /// The failure this catches is real and has happened twice in this project:
    /// a package list grew a new entry, nothing failed, and the name turned out
    /// to be wrong only at `pacstrap` inside a VM. Here it fails a unit test.
    #[test]
    fn every_installable_package_is_in_the_catalog() {
        let mut everything: Vec<String> = Vec::new();
        everything.extend(PackageSelection::default().core);
        for stack in [
            crate::services::AudioStack::None,
            crate::services::AudioStack::PipeWire,
            crate::services::AudioStack::PulseAudio,
        ] {
            everything.extend(
                crate::services::ServiceSelection {
                    audio: stack,
                    bluetooth: true,
                    ssh: true,
                    zram: true,
                    printing: true,
                    ..Default::default()
                }
                .packages(),
            );
        }
        for profile in crate::desktop::DesktopProfile::ALL {
            everything.extend(profile.packages());
        }
        everything.extend(crate::aur::AUR_HELPERS.iter().map(|s| s.to_string()));
        for name in everything {
            assert!(
                exists(&name),
                "`{name}` can be handed to pacstrap but is not in the catalog, \
                 so the search can never find it"
            );
        }
    }

    #[test]
    fn catalog_names_are_lowercase_and_unique() {
        let mut seen = std::collections::HashSet::new();
        for entry in CATALOG {
            assert_eq!(
                entry.name,
                entry.name.to_ascii_lowercase(),
                "package names are case sensitive on disk"
            );
            assert!(
                !entry.description.is_empty(),
                "{} has no description",
                entry.name
            );
            assert!(
                seen.insert(entry.name),
                "`{}` is listed twice in the catalog",
                entry.name
            );
        }
    }

    #[test]
    fn every_aur_helper_the_code_can_call_is_in_the_catalog_as_aur() {
        for helper in crate::aur::AUR_HELPERS {
            let entry = lookup(helper)
                .unwrap_or_else(|| panic!("{helper} is offered as a helper but not catalogued"));
            assert_eq!(entry.source, PackageSource::Aur, "{helper} must be AUR");
        }
    }

    /// The two names that have already cost this project a VM install.
    #[test]
    fn the_names_that_failed_in_a_vm_are_not_offered() {
        assert!(
            !exists("greetd-tuigreeter"),
            "typo: the package is greetd-tuigreet"
        );
        assert!(exists("greetd-tuigreet"));
        assert!(!exists("p7zip"), "p7zip left the Arch repositories");
        assert!(exists("7zip"), "7zip is what took its place");
    }

    #[test]
    fn a_search_never_invents_a_name() {
        // Everything the search can produce is a slice of the catalog, so this
        // is structural — but assert it anyway, because it is the guarantee the
        // whole feature rests on.
        for query in ["", "f", "zz", "a", "0", "-", "  "] {
            for aur in [false, true] {
                for name in searchable_names(query, None, aur) {
                    assert!(
                        CATALOG.iter().any(|e| e.name == name),
                        "search offered `{name}` for {query:?}"
                    );
                }
            }
        }
        assert!(searchable_names("definitely-not-a-package", None, true).is_empty());
        assert!(!exists("definitely-not-a-package"));
    }

    #[test]
    fn a_prefix_match_outranks_a_substring_match() {
        let hits = search("git", None, false);
        let names: Vec<&str> = hits.iter().map(|e| e.name).collect();
        assert_eq!(names.first(), Some(&"git"), "{names:?}");
    }

    #[test]
    fn the_search_finds_things_by_description_too() {
        let names = searchable_names("browser", None, false);
        assert!(names.contains(&"firefox".to_string()), "{names:?}");
        let names = searchable_names("microcode", None, false);
        assert!(names.contains(&"intel-ucode".to_string()), "{names:?}");
    }

    #[test]
    fn aur_packages_are_only_offered_when_aur_is_enabled() {
        let with = searchable_names("", None, true);
        let without = searchable_names("", None, false);
        assert!(with.contains(&"yay".to_string()), "{with:?}");
        assert!(!without.contains(&"yay".to_string()), "{without:?}");
        // Not merely hidden by a longer list: a direct search must not find it.
        assert!(searchable_names("yay", None, false).is_empty());
        assert_eq!(searchable_names("yay", None, true), vec!["yay".to_string()]);
        // Every AUR name is behind the flag, not just the helpers.
        for entry in CATALOG.iter().filter(|e| e.source == PackageSource::Aur) {
            assert!(
                !without.contains(&entry.name.to_string()),
                "{} is AUR and must need the flag",
                entry.name
            );
        }
    }

    #[test]
    fn a_profile_specific_package_is_not_offered_to_another_desktop() {
        assert!(searchable_names("xdg-desktop-portal", Some("gnome"), false)
            .contains(&"xdg-desktop-portal".to_string()));
        assert!(
            searchable_names("xdg-desktop-portal", Some("sway"), false).is_empty(),
            "Sway does not use the freedesktop portals"
        );
        assert!(
            searchable_names("xdg-desktop-portal", None, false).is_empty(),
            "a console-only profile installs no portal"
        );
    }

    #[test]
    fn lookup_ignores_case_and_surrounding_space() {
        assert_eq!(lookup("  Firefox ").map(|e| e.name), Some("firefox"));
        assert!(lookup("").is_none());
    }
}
