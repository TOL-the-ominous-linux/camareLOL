//! Application state and key handling for the camareLOL TUI.

use camarelol_core::boot::Bootloader;
use camarelol_core::config::InstallationConfig;
use camarelol_core::desktop::{DesktopProfile, DisplayServer};
use camarelol_core::error::{CamarelolError, ErrorCode};
use camarelol_core::hardware::Firmware;
use camarelol_core::install::{Outcome, ProgressEvent};
use camarelol_core::locale::{suggest, UiLanguage};
use camarelol_core::logging::Logger;
use camarelol_core::network::{self, Connectivity};
use camarelol_core::packages::PackageSelection;
use camarelol_core::safety::SafetyGate;
use camarelol_core::services::ServiceSelection;
use camarelol_core::storage::layout::{PartitionPlan, PartitionPurpose};
use camarelol_core::storage::{self, StorageDevice};
use std::path::PathBuf;

/// The steps of the installer, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Welcome,
    Language,
    Keyboard,
    Timezone,
    Storage,
    DiskMode,
    Layout,
    Filesystem,
    Accounts,
    Bootloader,
    Desktop,
    Services,
    Packages,
    Review,
    Progress,
    Done,
    Help,
    Logs,
}

impl Screen {
    pub const ORDER: [Screen; 16] = [
        Screen::Welcome,
        Screen::Language,
        Screen::Keyboard,
        Screen::Timezone,
        Screen::Storage,
        Screen::DiskMode,
        Screen::Layout,
        Screen::Filesystem,
        Screen::Accounts,
        Screen::Bootloader,
        Screen::Desktop,
        Screen::Services,
        Screen::Packages,
        Screen::Review,
        Screen::Progress,
        Screen::Done,
    ];

    pub fn title(&self) -> &'static str {
        match self {
            Screen::Welcome => "Welcome to camareLOL",
            Screen::Language => "Language",
            Screen::Keyboard => "Keyboard layout",
            Screen::Timezone => "Time zone",
            Screen::Storage => "Storage devices",
            Screen::DiskMode => "How should this disk be used?",
            Screen::Layout => "Partition layout",
            Screen::Filesystem => "Filesystem",
            Screen::Accounts => "Hostname and accounts",
            Screen::Bootloader => "Bootloader",
            Screen::Desktop => "Desktop environment",
            Screen::Services => "Services and storage extras",
            Screen::Packages => "Additional packages",
            Screen::Review => "Review plan",
            Screen::Progress => "Installing",
            Screen::Done => "Installation result",
            Screen::Help => "Help",
            Screen::Logs => "Installation log",
        }
    }

    pub fn index(&self) -> usize {
        Screen::ORDER.iter().position(|s| s == self).unwrap_or(0)
    }

    pub fn at(index: usize) -> Screen {
        Screen::ORDER[index.min(Screen::ORDER.len() - 1)]
    }
}

/// Which edit field has focus on a form screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    None,
    Hostname,
    RootPassword,
    RootPasswordConfirm,
    Username,
    UserPassword,
    UserPasswordConfirm,
    /// Not a text field: the row that gives the new user sudo/admin rights.
    Admin,
    CustomPackage,
}

pub struct App {
    pub core: camarelol_core::Core,
    pub screen: Screen,
    pub previous: Option<Screen>,
    pub quit: bool,
    pub firmware: Firmware,
    pub connectivity: Connectivity,
    /// Máy chỉ có Wi-Fi, chưa nối, và không có iwctl/nmtui để nối. Không có
    /// cờ này thì màn hình chỉ hiện "no network connection detected" — người
    /// dùng không được báo phải làm gì.
    pub wifi_needs_setup: bool,

    // cursor positions for the list screens
    pub storage_index: usize,
    pub disk_mode_index: usize,
    pub language_index: usize,
    pub keymap_index: usize,
    pub timezone_index: usize,
    pub fs_index: usize,
    pub bootloader_index: usize,
    pub desktop_index: usize,
    pub server_index: usize,
    pub layout_index: usize,
    pub services_cached_index: usize,
    pub package_index: usize,
    pub services: ServiceSelection,
    pub custom_packages: Vec<String>,
    pub custom_package_input: String,

    // the selected disk and its plan
    pub selected_disk: Option<String>,
    pub plan: Option<PartitionPlan>,

    // form state
    pub field: Field,
    pub hostname: String,
    pub root_password: String,
    pub root_password_confirm: String,
    pub username: String,
    pub user_password: String,
    pub user_password_confirm: String,
    pub user_is_admin: bool,
    pub confirm_phrase: String,

    // runtime
    pub message: Option<String>,
    pub error: Option<CamarelolError>,
    pub plan_log: Vec<ProgressEvent>,
    pub install_output: Vec<String>,
    pub report: Option<camarelol_core::install::execute::InstallReport>,
    pub outcome: Option<Outcome>,
    pub logger: Logger,
    pub save_config_path: Option<PathBuf>,
    pub log_path: String,
    pub armed: bool,
    pub execute_mode: bool,
    pub phase_progress: usize,
}

impl App {
    pub fn new(mock: Option<PathBuf>) -> Self {
        let logger = Logger::new(std::path::Path::new("/var/log/camarelol"));
        let core = match mock {
            Some(p) => {
                let devices =
                    storage::load_survey(p.to_str().unwrap_or_default()).unwrap_or_default();
                camarelol_core::Core::with_devices(devices)
            }
            None => camarelol_core::Core::new(camarelol_core::hardware::detect_firmware()),
        };
        let firmware = core.firmware();
        let connectivity = network::check_internet().unwrap_or(network::Connectivity::Down);
        let wifi_needs_setup = network::wifi_only_without_helper();
        let mut app = App {
            core,
            screen: Screen::Welcome,
            previous: None,
            quit: false,
            firmware,
            connectivity,
            wifi_needs_setup,
            storage_index: 0,
            disk_mode_index: 0,
            language_index: 1,
            keymap_index: 0,
            timezone_index: 0,
            fs_index: 0,
            bootloader_index: 0,
            desktop_index: 7,
            server_index: 0,
            layout_index: 0,
            services_cached_index: 0,
            package_index: 0,
            services: ServiceSelection::default(),
            custom_packages: Vec::new(),
            custom_package_input: String::new(),
            selected_disk: None,
            plan: None,
            field: Field::None,
            hostname: "archlinux".into(),
            root_password: String::new(),
            root_password_confirm: String::new(),
            username: String::new(),
            user_password: String::new(),
            user_password_confirm: String::new(),
            user_is_admin: true,
            confirm_phrase: String::new(),
            message: None,
            error: None,
            plan_log: Vec::new(),
            install_output: Vec::new(),
            report: None,
            outcome: None,
            logger,
            save_config_path: None,
            log_path: String::new(),
            armed: false,
            execute_mode: false,
            phase_progress: 0,
        };
        app.sync_language_to_system();
        app
    }

    /// Apply the UI language's suggested locale/keymap/timezone.
    pub fn sync_language_to_system(&mut self) {
        let lang = self.current_language();
        let plan = suggest(lang, None);
        self.core.config.system_locale = plan.locale;
        self.core.config.keymap = plan.keymap;
        self.core.config.timezone = plan.timezone;
    }

    pub fn current_language(&self) -> UiLanguage {
        if self.language_index == 0 {
            UiLanguage::Vietnamese
        } else {
            UiLanguage::English
        }
    }

    /// Localized text. Returns an owned String because both branches may be
    /// built at runtime (e.g. with a runtime value interpolated).
    pub fn tr(&self, vi: impl Into<String>, en: impl Into<String>) -> String {
        if self.current_language() == UiLanguage::Vietnamese {
            vi.into()
        } else {
            en.into()
        }
    }

    pub fn gate(&self) -> SafetyGate {
        if self.armed && self.execute_mode {
            SafetyGate::armed_and_execute()
        } else if self.armed {
            SafetyGate::armed()
        } else {
            SafetyGate::default()
        }
    }

    // ---------------------------------------------------------------- helpers

    pub fn devices(&self) -> &[StorageDevice] {
        &self.core.devices
    }

    pub fn device(&self, path: &str) -> Option<&StorageDevice> {
        self.core.device(path)
    }

    pub fn tr_owned(&self, vi: String, en: String) -> String {
        if self.current_language() == camarelol_core::locale::UiLanguage::Vietnamese {
            vi
        } else {
            en
        }
    }

    pub fn selected_device(&self) -> Option<&StorageDevice> {
        self.devices().get(self.storage_index)
    }

    pub fn rescan(&mut self) {
        match storage::scan_devices() {
            Ok(devices) => {
                self.core.devices = devices;
                if self.storage_index >= self.core.devices.len() {
                    self.storage_index = 0;
                }
                self.message = Some(self.tr("Đã quét lại ổ đĩa.", "Disks re-scanned."));
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Rebuild the partition plan from the current selections.
    pub fn rebuild_plan(&mut self) {
        let Some(path) = self.selected_disk.clone() else {
            self.message = Some(self.tr("Chưa chọn ổ đĩa.", "No disk selected yet."));
            return;
        };
        let Some(device) = self.core.device(&path).cloned() else {
            self.error = Some(CamarelolError::new(
                ErrorCode::DeviceGone,
                format!("device {path} disappeared"),
            ));
            return;
        };
        let req = camarelol_core::storage::layout::LayoutRequest {
            firmware: self.firmware,
            bootloader: self.bootloader(),
            root_filesystem: self.filesystem(),
            separate_home: self.layout_choice(PartitionPurpose::Home),
            separate_boot: self.layout_choice(PartitionPurpose::Boot),
            swap_bytes: if self.layout_choice(PartitionPurpose::Swap) {
                Some(self.zram_bytes())
            } else {
                None
            },
        };
        match camarelol_core::storage::layout::plan_auto_layout(&device, &req) {
            Ok(plan) => {
                if let Err(e) = self.core.validate_plan(&plan) {
                    self.error = Some(e);
                    self.plan = Some(plan);
                } else {
                    self.plan = Some(plan);
                }
            }
            Err(e) => self.error = Some(e),
        }
    }

    fn zram_bytes(&self) -> u64 {
        ServiceSelection::suggest_zram_bytes(camarelol_core::hardware::detect_ram_bytes())
    }

    fn layout_choice(&self, purpose: PartitionPurpose) -> bool {
        let want_home = self.layout_index == 1 || self.layout_index == 3;
        let want_boot = self.layout_index == 2 || self.layout_index == 3;
        match purpose {
            PartitionPurpose::Home => want_home,
            PartitionPurpose::Boot => want_boot,
            _ => false,
        }
    }

    pub fn filesystem(&self) -> camarelol_core::storage::Filesystem {
        use camarelol_core::storage::Filesystem;
        match self.fs_index {
            0 => Filesystem::Ext4,
            1 => Filesystem::Btrfs,
            2 => Filesystem::Xfs,
            _ => Filesystem::Ext4,
        }
    }

    pub fn filesystem_name(&self) -> String {
        self.filesystem().as_str().to_string()
    }

    pub fn bootloader(&self) -> Bootloader {
        // systemd-boot is UEFI only, so a BIOS machine always gets GRUB.
        if self.bootloader_index == 0 || self.firmware == Firmware::Bios {
            Bootloader::Grub
        } else {
            Bootloader::SystemdBoot
        }
    }

    pub fn desktop(&self) -> DesktopProfile {
        DesktopProfile::ALL[self.desktop_index.min(DesktopProfile::ALL.len() - 1)]
    }

    pub fn display_server(&self) -> DisplayServer {
        DisplayServer::ALL[self.server_index.min(DisplayServer::ALL.len() - 1)]
    }

    pub fn display_manager(&self) -> Option<&'static str> {
        self.desktop().display_managers().first().copied()
    }

    pub fn disk_modes(&self) -> Vec<&'static str> {
        vec![
            "Use the whole disk with an automatic layout",
            "Use existing partitions (keep their filesystems)",
            "Create a new layout, customising every partition",
            "Dual-boot next to another operating system",
            "Information only — change nothing",
        ]
    }

    /// Compile the TUI state into a validated configuration.
    pub fn build_config(&self) -> Result<InstallationConfig, CamarelolError> {
        let mut cfg = InstallationConfig {
            language: self.current_language().code().to_string(),
            ..Default::default()
        };
        cfg.system_locale = self.core.config.system_locale.clone();
        cfg.keymap = self.core.config.keymap.clone();
        cfg.timezone = self.core.config.timezone.clone();
        cfg.firmware = match self.firmware {
            Firmware::Uefi => "uefi",
            Firmware::Bios => "bios",
            Firmware::Unknown => "auto",
        }
        .to_string();
        if let Some(path) = &self.selected_disk {
            if let Some(d) = self.core.device(path) {
                cfg.target_disk = Some(camarelol_core::config::DiskReference {
                    path: path.clone(),
                    identity: d.identity(),
                    size_bytes: d.size_bytes,
                });
            }
        }
        cfg.filesystem = self.filesystem_name().to_string();
        cfg.separate_boot = self.layout_choice(PartitionPurpose::Boot);
        cfg.separate_home = self.layout_choice(PartitionPurpose::Home);
        cfg.swap_bytes = if self.layout_choice(PartitionPurpose::Swap) {
            Some(self.zram_bytes())
        } else {
            None
        };
        cfg.bootloader = match self.bootloader() {
            Bootloader::Grub => "grub",
            Bootloader::SystemdBoot => "systemd-boot",
        }
        .to_string();
        cfg.desktop = match self.desktop() {
            DesktopProfile::ConsoleOnly => "console",
            DesktopProfile::Plasma => "plasma",
            DesktopProfile::Gnome => "gnome",
            DesktopProfile::Xfce => "xfce",
            DesktopProfile::Cinnamon => "cinnamon",
            DesktopProfile::Lxqt => "lxqt",
            DesktopProfile::Hyprland => "hyprland",
            DesktopProfile::Sway => "sway",
        }
        .to_string();
        cfg.display_server = match self.display_server() {
            DisplayServer::Wayland => "wayland",
            DisplayServer::X11 => "x11",
            DisplayServer::Auto => "auto",
        }
        .to_string();
        cfg.display_manager = self.display_manager().map(|s| s.to_string());
        cfg.hostname = self.hostname.trim().to_string();
        cfg.root_login_allowed = true;
        if !self.username.trim().is_empty() {
            cfg.users = vec![self.username.trim().to_string()];
            if self.user_is_admin {
                cfg.admin_users = vec![self.username.trim().to_string()];
            }
        }
        let mut pkgs = PackageSelection {
            utilities: self.core_utility_packages(),
            drivers_firmware: self.driver_packages(),
            user_selected: self.custom_packages.clone(),
            // AUR stays off until the user explicitly turns it on.
            aur_enabled: false,
            ..PackageSelection::default()
        };
        pkgs.user_selected.sort();
        pkgs.user_selected.dedup();
        cfg.packages = pkgs;
        cfg.services = self.services.clone();
        cfg.set_passwords(
            Some(self.root_password.as_str()),
            &[(self.username.trim(), self.user_password.as_str())],
        );
        cfg.validate()?;
        Ok(cfg)
    }

    fn core_utility_packages(&self) -> Vec<String> {
        if self.current_language() == UiLanguage::Vietnamese {
            vec!["noto-fonts".into(), "noto-fonts-cjk".into(), "ibus".into()]
        } else {
            vec!["noto-fonts".into()]
        }
    }

    fn driver_packages(&self) -> Vec<String> {
        let mut v = vec!["mesa".into(), "linux-firmware".into()];
        let cpu = camarelol_core::hardware::detect_cpu();
        match cpu.microcode_hint {
            camarelol_core::hardware::MicrocodeHint::Intel => v.push("intel-ucode".into()),
            camarelol_core::hardware::MicrocodeHint::Amd => v.push("amd-ucode".into()),
            camarelol_core::hardware::MicrocodeHint::Unknown => {}
        }
        v
    }

    pub fn save_config_now(&self) {
        if let Some(path) = &self.save_config_path {
            if let Ok(cfg) = self.build_config() {
                let _ = cfg.save(path.to_str().unwrap_or_default());
            }
        }
    }
}
