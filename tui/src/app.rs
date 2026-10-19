//! Application state and key handling for the camareLOL TUI.

use camarelol_core::boot::Bootloader;
use camarelol_core::config::{InstallationConfig, ProfileLoad};
use camarelol_core::desktop::{DesktopProfile, DisplayServer};
use camarelol_core::error::{CamarelolError, ErrorCode};
use camarelol_core::hardware::Firmware;
use camarelol_core::install::{Outcome, ProgressEvent};
use camarelol_core::locale::{self, suggest, UiLanguage};
use camarelol_core::network::{self, Connectivity};
use camarelol_core::packages::{self, PackageEntry, PackageSelection};
use camarelol_core::recovery::RecoveryAdvice;
use camarelol_core::safety::SafetyGate;
use camarelol_core::services::ServiceSelection;
use camarelol_core::storage::layout::{PartitionPlan, PartitionPurpose};
use camarelol_core::storage::{self, Filesystem, StorageDevice};
use std::path::PathBuf;

/// The config/catalog name of a desktop profile. Shared by the wizard, the
/// config file and [`packages::search`], which filters catalog entries by it.
pub fn desktop_id_of(profile: DesktopProfile) -> &'static str {
    match profile {
        DesktopProfile::ConsoleOnly => "console",
        DesktopProfile::Plasma => "plasma",
        DesktopProfile::Gnome => "gnome",
        DesktopProfile::Xfce => "xfce",
        DesktopProfile::Cinnamon => "cinnamon",
        DesktopProfile::Lxqt => "lxqt",
        DesktopProfile::Hyprland => "hyprland",
        DesktopProfile::Sway => "sway",
    }
}

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
    /// The "load a saved profile" prompt on the welcome screen.
    LoadPath,
    /// The package search box.
    PackageSearch,
    /// The unprivileged account AUR packages are built as.
    AurBuildUser,
}

pub struct App {
    pub core: camarelol_core::Core,
    pub screen: Screen,
    pub previous: Option<Screen>,
    pub quit: bool,
    pub firmware: Firmware,
    /// Which distribution and init system this machine runs. Detected once at
    /// start-up — read-only, no process spawned — and shown on the welcome
    /// screen so a user on Artix or Hyperbola learns it before the wizard
    /// rather than at the moment the plan is refused.
    pub host: camarelol_core::host::HostProfile,
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
    /// Text of the package search box. Empty means "show the whole catalog",
    /// which is what the screen shows until somebody presses `/`.
    pub package_query: String,
    /// The unprivileged account AUR packages would be built as.
    ///
    /// Never guessed. camareLOL itself runs as root to write a disk, and this
    /// is the only thing that decides whose privileges run a PKGBUILD, so an
    /// empty value means "refuse", not "use root".
    pub aur_build_user: String,
    pub aur_enabled: bool,
    /// AUR packages the user switched on, kept apart from `custom_packages`
    /// because they are built after `pacstrap` and never handed to it.
    pub aur_selected: Vec<String>,

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
    /// Kênh nhận cập nhật từ thread cài đặt. `None` khi không cài gì.
    ///
    /// Cài đặt chạy ở thread nền vì nó mất hàng chục giây tới hàng chục phút.
    /// Chạy đồng bộ trong `handle_key` nghĩa là vòng lặp chính không quay lại
    /// để vẽ, nên màn Progress không bao giờ hiện, và không phím nào nhận
    /// được kể cả Ctrl+C — nhìn từ ngoài như treo.
    pub install_rx: Option<std::sync::mpsc::Receiver<crate::keys::InstallUpdate>>,
    /// Đang cài: dùng để chặn thao tác không an toàn giữa chừng.
    pub installing: bool,
    pub report: Option<camarelol_core::install::execute::InstallReport>,
    pub outcome: Option<Outcome>,
    pub save_config_path: Option<PathBuf>,
    /// Where the saved profile the wizard is showing came from, so the user can
    /// see that they are editing somebody's earlier answers and not defaults.
    pub loaded_from: Option<PathBuf>,
    /// What is being typed into the "load a saved profile" prompt.
    pub load_path_input: String,
    /// `--save-diagnostics`: where a report is written when asked for.
    pub diagnostics_path: Option<PathBuf>,
    /// Recovery guidance, filled in when an install stops early.
    pub recovery: Option<RecoveryAdvice>,
    /// Set once the user has moved the keyboard or timezone cursor, so an
    /// untouched screen does not overwrite a loaded profile with the list
    /// default.
    pub keymap_edited: bool,
    pub timezone_edited: bool,
    pub log_path: String,
    pub armed: bool,
    pub execute_mode: bool,
    /// `--preview`: simulate the whole installation, print every command,
    /// write nothing. Set once from the command line and never cleared, so
    /// there is no key press that can turn a preview into a real install.
    pub preview: bool,
    pub phase_progress: usize,
}

impl App {
    pub fn new(mock: Option<PathBuf>) -> Self {
        let core = match mock {
            Some(p) => {
                let devices =
                    storage::load_survey(p.to_str().unwrap_or_default()).unwrap_or_default();
                camarelol_core::Core::with_devices(devices)
            }
            None => camarelol_core::Core::new(camarelol_core::hardware::detect_firmware()),
        };
        let firmware = core.firmware();
        // Read-only: reads /etc/os-release and /proc, starts no process, so it
        // behaves identically in a `--preview` run.
        let host = camarelol_core::host::detect();
        let connectivity = network::check_internet().unwrap_or(network::Connectivity::Down);
        let wifi_needs_setup = network::wifi_only_without_helper();
        let mut app = App {
            core,
            screen: Screen::Welcome,
            previous: None,
            quit: false,
            firmware,
            host,
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
            package_query: String::new(),
            aur_build_user: String::new(),
            aur_enabled: false,
            aur_selected: Vec::new(),
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
            install_rx: None,
            installing: false,

            report: None,
            outcome: None,
            save_config_path: None,
            loaded_from: None,
            load_path_input: String::new(),
            diagnostics_path: None,
            recovery: None,
            keymap_edited: false,
            timezone_edited: false,
            log_path: String::new(),
            armed: false,
            execute_mode: false,
            preview: false,
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
        // The cursor is an index into `UiLanguage::at`, which clamps — so a
        // profile written before Chinese existed cannot leave it out of range.
        UiLanguage::at(self.language_index)
    }

    /// Localized text. Returns an owned String because a branch may be built at
    /// runtime (e.g. with a runtime value interpolated).
    ///
    /// Third argument is Chinese; the pair order is vi, en, zh, matching
    /// `UiLanguage`'s declaration order.
    pub fn tr(
        &self,
        vi: impl Into<String>,
        en: impl Into<String>,
        zh: impl Into<String>,
    ) -> String {
        match self.current_language() {
            UiLanguage::Vietnamese => vi.into(),
            UiLanguage::English => en.into(),
            UiLanguage::Chinese => zh.into(),
        }
    }

    pub fn gate(&self) -> SafetyGate {
        // Preview wins over both flags, and it is checked first on purpose: a
        // user who typed `--preview --allow-destructive --execute` (or an old
        // wrapper script that appends them automatically) still gets a run that
        // cannot write. There is no combination of arguments that reaches the
        // execute gate from here.
        if self.preview {
            return SafetyGate::preview();
        }
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

    pub fn tr_owned(
        &self,
        vi: impl Into<String>,
        en: impl Into<String>,
        zh: impl Into<String>,
    ) -> String {
        self.tr(vi, en, zh)
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
                self.message = Some(self.tr(
                    "Đã quét lại ổ đĩa.",
                    "Disks re-scanned.",
                    "已重新扫描磁盘。",
                ));
            }
            Err(e) => self.error = Some(e),
        }
    }

    /// Rebuild the partition plan from the current selections.
    pub fn rebuild_plan(&mut self) {
        let Some(path) = self.selected_disk.clone() else {
            self.message = Some(self.tr(
                "Chưa chọn ổ đĩa.",
                "No disk selected yet.",
                "尚未选择磁盘。",
            ));
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

    /// The identifier this desktop profile has in a configuration file and in
    /// the package catalog. One table, so a profile name and a package name
    /// can never drift apart.
    pub fn desktop_id(&self) -> &'static str {
        desktop_id_of(self.desktop())
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
        cfg.desktop = self.desktop_id().to_string();
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
            // AUR stays off until the user explicitly turns it on, and its
            // packages are only ever built as the named unprivileged account.
            aur_enabled: self.aur_enabled,
            aur_packages: self.aur_selected.clone(),
            aur_build_user: self.aur_build_user_name().map(str::to_string),
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
        // Noto CJK and an input method are not decoration: without them a
        // Vietnamese or Chinese console shows boxes for every accented or Han
        // character and there is no way to type.
        match self.current_language() {
            UiLanguage::Vietnamese => {
                vec!["noto-fonts".into(), "noto-fonts-cjk".into(), "ibus".into()]
            }
            UiLanguage::Chinese => {
                vec!["noto-fonts".into(), "noto-fonts-cjk".into(), "ibus".into()]
            }
            UiLanguage::English => vec!["noto-fonts".into()],
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

    // ------------------------------------------------------------------
    // Load a saved profile
    // ------------------------------------------------------------------

    /// Load a profile from a path and make it the state of the wizard.
    ///
    /// Three outcomes, and they are deliberately different:
    ///
    /// * a valid profile — every choice it holds is applied and the wizard
    ///   shows it;
    /// * a file with nothing in it — the wizard keeps its defaults and says so,
    ///   because `touch profile.toml` is not an error;
    /// * anything else — a refusal, shown as an error. Never a quiet fall back
    ///   to defaults: a profile that was asked for and half applied is how
    ///   somebody ends up installing with the wrong disk or the wrong locale
    ///   and finding out afterwards.
    ///
    /// Passwords are not part of the file and are not restored here; see
    /// [`camarelol_core::config::InstallationConfig`]'s `#[serde(skip)]`.
    pub fn load_profile(&mut self, path: &str) -> Result<(), CamarelolError> {
        let cfg = match InstallationConfig::load_profile(path)? {
            ProfileLoad::Empty => {
                self.loaded_from = None;
                self.message = Some(self.tr_owned(
                    format!("{path} has no profile in it — starting fresh."),
                    format!("{path} holds no profile — starting fresh."),
                    format!("{path} 中没有配置 — 已重新开始。"),
                ));
                return Ok(());
            }
            ProfileLoad::Restored(cfg) => *cfg,
        };
        let note = self.apply_config(&cfg)?;
        self.loaded_from = Some(PathBuf::from(path));
        self.message = Some(self.tr_owned(
            format!("Đã nạp cấu hình từ {path}. {note}"),
            format!("Loaded the profile from {path}. {note}"),
            format!("已从 {path} 加载配置。{note}"),
        ));
        Ok(())
    }

    /// Populate the wizard from a profile that has already been read *and
    /// validated* by [`InstallationConfig::load_profile`].
    ///
    /// Returns a short note about anything that could not be carried over
    /// faithfully — a disk that is not attached, a layout that no longer fits
    /// the attached disk — so the caller can show it. Values the wizard cannot
    /// represent at all are an error rather than a substitution.
    pub fn apply_config(&mut self, cfg: &InstallationConfig) -> Result<String, CamarelolError> {
        // The core keeps its own copy and the layout validator reads *that*,
        // so it has to be replaced first or the plan would be checked against
        // the defaults.
        self.core.config = cfg.clone();

        let mut notes: Vec<String> = Vec::new();

        self.language_index = cfg.ui_language()?.index();
        // Locale, keymap and timezone are free-form strings in the file, so a
        // profile may hold one the three-item picker does not list. The value
        // is kept exactly; the cursor just sits on the first row, and the
        // `*_edited` flags make sure walking past the screen does not
        // overwrite it.
        let keymap_known = locale::KEYMAPS.iter().any(|k| *k == cfg.keymap);
        let keymap_index = locale::KEYMAPS
            .iter()
            .position(|k| *k == cfg.keymap)
            .unwrap_or(0);
        let timezone_known = locale::TIMEZONES.iter().any(|t| *t == cfg.timezone);
        let timezone_index = locale::TIMEZONES
            .iter()
            .position(|t| *t == cfg.timezone)
            .unwrap_or(0);
        self.keymap_index = keymap_index;
        self.timezone_index = timezone_index;
        self.keymap_edited = false;
        self.timezone_edited = false;
        self.core.config.keymap = cfg.keymap.clone();
        self.core.config.timezone = cfg.timezone.clone();
        self.core.config.system_locale = cfg.system_locale.clone();
        if !keymap_known {
            notes.push(self.tr_owned(
                format!(
                    "keymap `{}` is not in the list; it is kept as it is.",
                    cfg.keymap
                ),
                format!(
                    "keymap `{}` is not one of the listed layouts; it is kept as it is.",
                    cfg.keymap
                ),
                format!("键盘布局 `{}` 不在列表中 — 已按原值保留。", cfg.keymap),
            ));
        }
        if !timezone_known {
            notes.push(self.tr_owned(
                format!(
                    "time zone `{}` is not in the list; it is kept as it is.",
                    cfg.timezone
                ),
                format!(
                    "time zone `{}` is not one of the listed zones; kept as it is.",
                    cfg.timezone
                ),
                format!("时区 `{}` 不在列表中 — 已按原值保留。", cfg.timezone),
            ));
        }

        // Filesystem: the wizard offers ext4, btrfs and xfs. A profile naming
        // anything else is refused rather than quietly installed as something
        // the user did not ask for.
        self.fs_index = match cfg.filesystem_type()? {
            Filesystem::Ext4 => 0,
            Filesystem::Btrfs => 1,
            Filesystem::Xfs => 2,
            other => {
                return Err(CamarelolError::new(
                    ErrorCode::NotSupported,
                    format!(
                        "the profile asks for the `{}` filesystem, which this installer cannot create",
                        other.as_str()
                    ),
                )
                .with_remediation(
                    "edit the profile to say ext4, btrfs or xfs, or choose the filesystem again in the wizard",
                ))
            }
        };

        self.layout_index = match (cfg.separate_home, cfg.separate_boot) {
            (false, false) => 0,
            (true, false) => 1,
            (false, true) => 2,
            (true, true) => 3,
        };
        // `swap_bytes` has no row of its own on the layout screen — the layout
        // list only ever offered root/home/boot. Carrying it into zram keeps the
        // choice instead of dropping it on the floor.
        if let Some(bytes) = cfg.swap_bytes {
            self.services.zram = true;
            self.services.zram_size_mb = Some((bytes / (1024 * 1024)).min(u32::MAX as u64) as u32);
        }

        self.bootloader_index = match cfg.bootloader()? {
            Bootloader::Grub => 0,
            Bootloader::SystemdBoot => 1,
        };
        self.core.set_bootloader(self.bootloader());

        self.desktop_index = DesktopProfile::ALL
            .iter()
            .position(|p| desktop_id_of(*p) == cfg.desktop)
            .unwrap_or(DesktopProfile::ALL.len() - 1);
        self.server_index = match cfg.display_server()? {
            DisplayServer::Auto => 0,
            DisplayServer::Wayland => 1,
            DisplayServer::X11 => 2,
        };

        self.services = cfg.services.clone();
        self.custom_packages = cfg.packages.user_selected.clone();
        self.package_query.clear();
        self.aur_enabled = cfg.packages.aur_enabled;
        self.aur_build_user = cfg.packages.aur_build_user.clone().unwrap_or_default();
        // AUR entries are only restored when AUR is on: keeping them while the
        // flag is off would leave a profile whose `aur_packages` list is
        // invisible and unremovable from the screen.
        self.aur_selected = if cfg.packages.aur_enabled {
            cfg.packages.aur_packages.clone()
        } else {
            Vec::new()
        };

        self.hostname = cfg.hostname.clone();
        self.username = cfg.users.first().cloned().unwrap_or_default();
        self.user_is_admin = cfg
            .users
            .first()
            .map(|u| cfg.admin_users.contains(u))
            .unwrap_or(false);
        // Passwords are never restored, by construction and by policy: there is
        // no assignment here, and there is no field on the file to assign from.
        self.root_password.clear();
        self.root_password_confirm.clear();
        self.user_password.clear();
        self.user_password_confirm.clear();

        // The disk is a hint and never a fact: it is re-identified from the
        // current survey, and a disk that is not there is reported rather than
        // substituted with whatever happens to be first in the list.
        self.selected_disk = None;
        if let Some(reference) = &cfg.target_disk {
            match self.core.device(&reference.path) {
                Some(device) if device.identity() == reference.identity => {
                    self.selected_disk = Some(reference.path.clone());
                    self.storage_index = self
                        .core
                        .devices
                        .iter()
                        .position(|d| d.path == reference.path)
                        .unwrap_or(0);
                    self.rebuild_plan();
                }
                Some(device) => {
                    notes.push(self.tr_owned(
                        format!(
                            "{} is attached but is a different disk now ({}); nothing was selected.",
                            reference.path,
                            device.identity()
                        ),
                        format!(
                            "{} is attached but is a different disk now ({}); nothing was selected.",
                            reference.path,
                            device.identity()
                        ),
                        format!(
                            "{} 已连接，但现在是另一块磁盘（{}）；未选择任何磁盘。",
                            reference.path,
                            device.identity()
                        ),
                    ));
                }
                None => notes.push(self.tr_owned(
                    format!("{} is not attached; choose a disk again.", reference.path),
                    format!(
                        "{} from that profile is not attached — choose a disk again.",
                        reference.path
                    ),
                    format!("配置中的 {} 未连接 — 请重新选择磁盘。", reference.path),
                )),
            }
        }

        Ok(if notes.is_empty() {
            self.tr_owned(
                "Every choice was restored.",
                "Every choice was restored.",
                "已恢复所有选项。",
            )
        } else {
            notes.join(" ")
        })
    }

    // ------------------------------------------------------------------
    // Packages
    // ------------------------------------------------------------------

    /// The catalog rows the packages screen shows, given the search text, the
    /// chosen desktop and whether AUR is on.
    ///
    /// Every name in here comes from [`camarelol_core::packages::CATALOG`]. The
    /// screen cannot show a package that is not in the catalog, which is the
    /// whole point: a name typed by hand is still allowed (it is the user's
    /// call), but nothing here can invent one.
    pub fn package_rows(&self) -> Vec<&'static PackageEntry> {
        packages::search(
            &self.package_query,
            Some(self.desktop_id()),
            self.aur_enabled,
        )
    }

    /// Is this package currently selected?
    pub fn package_is_selected(&self, name: &str) -> bool {
        self.custom_packages.iter().any(|p| p == name)
            || self.aur_packages().iter().any(|p| p == name)
    }

    /// AUR packages the user has switched on. Empty unless AUR is enabled.
    pub fn aur_packages(&self) -> Vec<String> {
        if self.aur_enabled {
            self.aur_selected.clone()
        } else {
            Vec::new()
        }
    }

    /// The unprivileged account AUR packages would be built as, if any.
    pub fn aur_build_user_name(&self) -> Option<&str> {
        let name = self.aur_build_user.trim();
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }
}
