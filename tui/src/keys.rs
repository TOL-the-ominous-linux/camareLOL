//! Keyboard handling and screen navigation for the installer.
//!
//! Tệp này từng tên là `update.rs`, vì phần kiểm tra bản cập nhật được viết
//! trước và mọi thứ sau đó cứ nối vào đó. Nó không liên quan gì tới cập nhật.
//! `handle_key` — toàn bộ bàn phím của giao diện — nằm ở đây, và xoá tệp kia
//! để bỏ updater đã xoá luôn bàn phím. Tên mới nói đúng nội dung.

use super::app::{App, Field, Screen};
use camarelol_core::error::ErrorCode;
use camarelol_core::install::execute::RealSystemRunner;
use camarelol_core::install::{run_install, Outcome, PhaseState, ProgressEvent, ProgressSink};
use camarelol_core::safety::{required_phrase, Operation};
use camarelol_core::storage::RiskLevel;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A sink that streams events into the App so the UI can render them.
pub struct AppSink<'a> {
    pub app: &'a mut App,
}

impl<'a> ProgressSink for AppSink<'a> {
    fn emit(&mut self, event: ProgressEvent) {
        match &event {
            ProgressEvent::PhaseStarted { name } => {
                self.app.install_output.push(format!(
                    "▶ {} — {}",
                    name,
                    camarelol_core::install::describe_phase(name)
                ));
                self.app.logger.log("install", &format!("started {name}"));
            }
            ProgressEvent::PhaseFinished { name, state } => {
                self.app
                    .install_output
                    .push(format!("■ {name} — {}", state_str(state)));
                self.app
                    .logger
                    .log("install", &format!("finished {name}: {}", state_str(state)));
                self.app.phase_progress += 1;
            }
            ProgressEvent::CommandPreview { command } => {
                self.app.install_output.push(format!("  $ {command}"));
                self.app.logger.log("install", command);
            }
            ProgressEvent::Info { message } => {
                self.app.install_output.push(format!("  · {message}"));
            }
        }
        self.app.plan_log.push(event);
    }
}

fn state_str(s: &PhaseState) -> &'static str {
    s.as_str()
}

impl App {
    pub fn handle_key(&mut self, key: KeyEvent) {
        // global keys first
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.quit = true;
                return;
            }
            // F1 toggles the help overlay, so it can be closed with the same
            // key that opened it.
            KeyCode::F(1) => {
                if self.screen == Screen::Help {
                    self.screen = self.previous.unwrap_or(Screen::Welcome);
                } else {
                    self.previous = Some(self.screen);
                    self.screen = Screen::Help;
                }
                return;
            }
            // Esc goes back one step everywhere it is advertised. Handled
            // globally because several screens used to drop it, which made
            // them one-way ratchets even though the footer offered a way back.
            KeyCode::Esc if self.can_go_back() => {
                self.go_back();
                return;
            }
            KeyCode::F(10) => {
                self.rebuild_plan();
                self.screen = Screen::Review;
                return;
            }
            KeyCode::F(5) => {
                self.rescan();
                return;
            }
            _ => {}
        }

        match self.screen {
            Screen::Help | Screen::Logs => {
                if key.code == KeyCode::Esc {
                    self.screen = self.previous.unwrap_or(Screen::Welcome);
                }
                return;
            }
            _ => {}
        }

        match self.screen {
            Screen::Welcome => {
                if key.code == KeyCode::Enter {
                    self.go_next();
                }
            }
            Screen::Language
            | Screen::Keyboard
            | Screen::Timezone
            | Screen::Filesystem
            | Screen::Bootloader => match key.code {
                KeyCode::Up => self.move_cursor(-1),
                KeyCode::Down => self.move_cursor(1),
                KeyCode::Enter => {
                    if self.screen == Screen::Language {
                        self.sync_language_to_system();
                    }
                    if self.screen == Screen::Bootloader {
                        self.rebuild_plan();
                    }
                    self.go_next();
                }
                _ => {}
            },
            Screen::Storage => match key.code {
                KeyCode::Up => self.move_cursor(-1),
                KeyCode::Down => self.move_cursor(1),
                KeyCode::Enter => {
                    if let Some(dev) = self.selected_device().cloned() {
                        self.selected_disk = Some(dev.path.clone());
                        self.core.set_bootloader(self.bootloader());
                        self.rebuild_plan();
                        self.screen = Screen::DiskMode;
                    }
                }
                KeyCode::Char('i') => {
                    self.screen = Screen::Logs;
                }
                _ => {}
            },
            Screen::DiskMode => match key.code {
                KeyCode::Up => self.disk_mode_index = self.disk_mode_index.saturating_sub(1),
                KeyCode::Down => {
                    let modes = self.disk_modes();
                    if self.disk_mode_index + 1 < modes.len() {
                        self.disk_mode_index += 1;
                    }
                }
                KeyCode::Enter => {
                    if self.disk_mode_index == 4 {
                        self.screen = Screen::Storage;
                        return;
                    }
                    if self.disk_mode_index == 1 {
                        self.message = Some(self.tr(
                            "Chế độ dùng phân vùng sẵn có chưa được triển khai ở phiên bản này — đã chặn để bảo vệ dữ liệu.",
                            "Re-using existing partitions is not implemented in this version — the operation was blocked to protect your data.",
                        ));
                        self.error = Some(camarelol_core::error::CamarelolError::new(
                            ErrorCode::NotSupported,
                            "existing-partition reuse requires the advanced storage workflow",
                        ));
                        return;
                    }
                    if self.disk_mode_index == 3 {
                        self.message = Some(self.tr(
                            "Cài song song sẽ được bật sau khi phát hiện Windows — hiện chỉ có bố cục tự động an toàn.",
                            "Dual-boot is enabled after Windows is detected — only the safe automatic layout is available now.",
                        ));
                    }
                    self.rebuild_plan();
                    self.go_next();
                }
                _ => {}
            },
            Screen::Layout => match key.code {
                KeyCode::Up => self.layout_index = self.layout_index.saturating_sub(1),
                KeyCode::Down => self.layout_index = (self.layout_index + 1).min(3),
                // Enter accepts the shown layout and moves on. Previously it
                // only rebuilt the plan, which left users stuck on this screen.
                KeyCode::Enter | KeyCode::Right => {
                    self.rebuild_plan();
                    self.go_next();
                }
                _ => {}
            },
            Screen::Services => match key.code {
                KeyCode::Up => self.service_cursor_up(),
                KeyCode::Down => self.service_cursor_down(),
                KeyCode::Char(' ') => self.toggle_service(),
                KeyCode::Enter => self.go_next(),
                _ => {}
            },
            Screen::Desktop => match key.code {
                KeyCode::Up => self.desktop_index = self.desktop_index.saturating_sub(1),
                KeyCode::Down => {
                    self.desktop_index = (self.desktop_index + 1).min(7);
                    self.server_index = 0;
                }
                KeyCode::Right | KeyCode::Tab => self.server_index = (self.server_index + 1) % 3,
                KeyCode::Enter => self.go_next(),
                _ => {}
            },
            Screen::Packages => match key.code {
                KeyCode::Char(c) if self.field == Field::CustomPackage => {
                    self.custom_package_input.push(c);
                }
                KeyCode::Backspace if self.field == Field::CustomPackage => {
                    self.custom_package_input.pop();
                }
                KeyCode::Enter if self.field == Field::CustomPackage => {
                    if !self.custom_package_input.trim().is_empty() {
                        let name = self.custom_package_input.trim().to_string();
                        if camarelol_core::accounts::validate_username(&name).is_ok() {
                            self.custom_packages.push(name);
                        }
                        self.custom_package_input.clear();
                    }
                }
                KeyCode::Up => self.packages_cursor_up(),
                KeyCode::Down => self.packages_cursor_down(),
                KeyCode::Char(' ') => self.toggle_package_suggestion(),
                KeyCode::Tab => {
                    self.field = match self.field {
                        Field::CustomPackage => Field::None,
                        _ => Field::CustomPackage,
                    };
                }
                // Outside the "add package" field, Enter means "I'm done here"
                // — otherwise this screen was a dead end.
                KeyCode::Right => self.go_next(),
                KeyCode::Enter => {
                    self.field = Field::None;
                    self.go_next();
                }
                _ => {}
            },
            Screen::Accounts => self.handle_accounts_key(key),
            Screen::Review => match key.code {
                KeyCode::Enter => self.try_start_install(),
                KeyCode::Char('s') => self.save_config_now(),
                _ => {}
            },
            Screen::Progress => {
                // nothing: install runs synchronously and moves to Done
            }
            Screen::Done if key.code == KeyCode::Enter => {
                self.quit = true;
            }
            _ => {}
        }
    }

    fn go_next(&mut self) {
        let next = self.screen.index() + 1;
        self.screen = Screen::at(next);
        self.focus_default_field();
    }

    /// Put the cursor somewhere the next keystroke will land.
    ///
    /// Arriving at a form with no field focused meant the very first
    /// keystrokes went nowhere: the screen looked frozen until you happened to
    /// press Tab.
    fn focus_default_field(&mut self) {
        if self.screen == Screen::Accounts {
            self.field = Field::Hostname;
        }
    }

    /// Whether Esc should move to the previous step here.
    ///
    /// Not on the first screen (nowhere to go), not once the install is
    /// running (going back would desync the progress log from reality).
    fn can_go_back(&self) -> bool {
        let i = self.screen.index();
        i > 0 && i < Screen::Progress.index()
    }

    fn go_back(&mut self) {
        if !self.can_go_back() {
            return;
        }
        self.previous = Some(self.screen);
        self.screen = Screen::at(self.screen.index() - 1);
        // A text field must not keep the cursor it had on the screen we left.
        self.field = Field::None;
        self.focus_default_field();
    }

    fn move_cursor(&mut self, delta: i32) {
        // The storage screen has its own cursor. It used to fall through to the
        // default arm and silently move the *language* selection instead, so the
        // arrow keys appeared to do nothing there.
        let count = match self.screen {
            Screen::Language => 2,
            Screen::Keyboard => camarelol_core::locale::KEYMAPS.len(),
            Screen::Timezone => camarelol_core::locale::TIMEZONES.len(),
            Screen::Filesystem => 3,
            Screen::Bootloader => 2,
            Screen::Storage => self.devices().len().max(1),
            _ => 1,
        };
        let current = match self.screen {
            Screen::Language => &mut self.language_index,
            Screen::Keyboard => &mut self.keymap_index,
            Screen::Timezone => &mut self.timezone_index,
            Screen::Filesystem => &mut self.fs_index,
            Screen::Bootloader => &mut self.bootloader_index,
            Screen::Storage => &mut self.storage_index,
            _ => &mut self.language_index,
        };
        let mut idx = *current as i32 + delta;
        if idx < 0 {
            idx = 0;
        }
        if idx >= count as i32 {
            idx = count as i32 - 1;
        }
        *current = idx as usize;
    }

    // ------------------------------------------------------------- accounts

    /// The rows the account form cursor moves between, in display order.
    fn account_fields() -> [Field; 7] {
        [
            Field::Hostname,
            Field::RootPassword,
            Field::RootPasswordConfirm,
            Field::Username,
            Field::UserPassword,
            Field::UserPasswordConfirm,
            Field::Admin,
        ]
    }

    fn move_account_field(&mut self, delta: i32) {
        let fields = Self::account_fields();
        let pos = fields.iter().position(|f| *f == self.field).unwrap_or(0) as i32;
        let next = (pos + delta).rem_euclid(fields.len() as i32) as usize;
        self.field = fields[next];
    }

    fn handle_accounts_key(&mut self, key: KeyEvent) {
        match key.code {
            // Up/Down used to fall through to `_ => {}`: the footer promised
            // they moved, and typing did nothing until Tab had been pressed.
            KeyCode::Down | KeyCode::Tab => self.move_account_field(1),
            KeyCode::Up | KeyCode::BackTab => self.move_account_field(-1),
            KeyCode::Right if self.field == Field::Admin => {
                self.user_is_admin = !self.user_is_admin
            }
            KeyCode::Left if self.field == Field::Admin => self.user_is_admin = !self.user_is_admin,
            KeyCode::Char(' ') if self.field == Field::Admin => {
                self.user_is_admin = !self.user_is_admin;
            }
            KeyCode::Backspace => match self.field {
                Field::Hostname => {
                    self.hostname.pop();
                }
                Field::RootPassword => {
                    self.root_password.pop();
                }
                Field::RootPasswordConfirm => {
                    self.root_password_confirm.pop();
                }
                Field::Username => {
                    self.username.pop();
                }
                Field::UserPassword => {
                    self.user_password.pop();
                }
                Field::UserPasswordConfirm => {
                    self.user_password_confirm.pop();
                }
                _ => {}
            },
            KeyCode::Char(c) => match self.field {
                Field::Hostname => self.hostname.push(c),
                Field::RootPassword => self.root_password.push(c),
                Field::RootPasswordConfirm => self.root_password_confirm.push(c),
                Field::Username => self.username.push(c),
                Field::UserPassword => self.user_password.push(c),
                Field::UserPasswordConfirm => self.user_password_confirm.push(c),
                _ => {}
            },
            KeyCode::Enter => {
                if !self.hostname.trim().is_empty() {
                    self.core.config.hostname = self.hostname.trim().to_string();
                }
                // The belt: everything is validated again when the plan runs.
                match self.build_config() {
                    Ok(cfg) => {
                        self.core.config.users = cfg.users.clone();
                        self.core.config.admin_users = cfg.admin_users.clone();
                        self.core.config.hostname = cfg.hostname.clone();
                        self.core.config.root_login_allowed = true;
                        self.go_next();
                    }
                    Err(e) => self.error = Some(e),
                }
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------- services

    fn service_cursor_up(&mut self) {
        let i = self.service_index();
        self.set_service_index(i.saturating_sub(1));
    }

    fn service_cursor_down(&mut self) {
        let i = self.service_index();
        self.set_service_index((i + 1).min(6));
    }

    pub fn service_index(&self) -> usize {
        self.services_cached_index
    }

    fn set_service_index(&mut self, i: usize) {
        self.services_cached_index = i;
    }

    fn toggle_service(&mut self) {
        match self.service_index() {
            0 => self.services.network_manager = !self.services.network_manager,
            1 => self.services.bluetooth = !self.services.bluetooth,
            2 => {
                self.services.audio = match self.services.audio {
                    camarelol_core::services::AudioStack::PipeWire => {
                        camarelol_core::services::AudioStack::PulseAudio
                    }
                    camarelol_core::services::AudioStack::PulseAudio => {
                        camarelol_core::services::AudioStack::None
                    }
                    camarelol_core::services::AudioStack::None => {
                        camarelol_core::services::AudioStack::PipeWire
                    }
                }
            }
            3 => self.services.zram = !self.services.zram,
            4 => self.services.printing = !self.services.printing,
            5 => self.services.ssh = !self.services.ssh,
            _ => {}
        }
    }

    // ------------------------------------------------------------- packages

    pub fn package_suggestions(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("htop", "process viewer"),
            ("firefox", "web browser"),
            ("git", "version control"),
            ("neofetch", "system info"),
            ("btop", "resource monitor"),
            ("reflector", "mirror ranking tool"),
        ]
    }

    fn packages_cursor_up(&mut self) {
        self.package_index = self.package_index.saturating_sub(1);
    }

    fn packages_cursor_down(&mut self) {
        self.package_index = (self.package_index + 1).min(self.package_suggestions().len() - 1);
    }

    fn toggle_package_suggestion(&mut self) {
        let suggestions = self.package_suggestions();
        if let Some((name, _)) = suggestions.get(self.package_index) {
            let name = name.to_string();
            if let Some(pos) = self.custom_packages.iter().position(|p| *p == name) {
                self.custom_packages.remove(pos);
            } else {
                self.custom_packages.push(name);
            }
        }
    }

    // ------------------------------------------------------------- install

    /// Start the installation after re-validating everything.
    pub fn try_start_install(&mut self) {
        self.error = None;
        let Some(path) = self.selected_disk.clone() else {
            self.error = Some(camarelol_core::error::CamarelolError::new(
                ErrorCode::NoTargetDisk,
                "no target disk selected",
            ));
            return;
        };
        let Some(device) = self.core.device(&path).cloned() else {
            self.error = Some(camarelol_core::error::CamarelolError::new(
                ErrorCode::DeviceGone,
                format!("{path} is gone"),
            ));
            return;
        };
        // Re-scan and verify the device identity right before anything happens.
        match camarelol_core::storage::rescan_and_verify(&device) {
            Ok(fresh) => {
                self.core.devices = self
                    .core
                    .devices
                    .iter()
                    .map(|d| {
                        if d.path == fresh.path {
                            fresh.clone()
                        } else {
                            d.clone()
                        }
                    })
                    .collect();
            }
            Err(e) => {
                self.error = Some(e);
                return;
            }
        }

        let cfg = match self.build_config() {
            Ok(c) => c,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        let plan = match self.plan.clone() {
            Some(p) => p,
            None => {
                self.error = Some(camarelol_core::error::CamarelolError::new(
                    ErrorCode::InvalidPlan,
                    "no partition plan available; open the layout step again",
                ));
                return;
            }
        };
        if let Err(e) = self.core.validate_plan(&plan) {
            self.error = Some(e);
            return;
        }

        if !self.armed {
            self.error = Some(camarelol_core::error::CamarelolError::new(
                ErrorCode::SafetyGateClosed,
                "destructive mode is not enabled; restart camareLOL with --allow-destructive",
            ));
            return;
        }

        // Confirm the destructive operations with the typed phrase.
        let mut gate = self.gate();
        let risk = plan.disk_risk;
        match gate.confirm(Operation::WritePartitionTable, risk, &self.confirm_phrase) {
            Ok(()) => {}
            Err(e) => {
                self.error = Some(e);
                return;
            }
        }
        // Record confirmations for every operation the install performs, at every
        // risk level: confirmations are keyed by (operation, risk).
        for op in [
            Operation::FormatPartition,
            Operation::InstallBootloader,
            Operation::Pacstrap,
            Operation::Mount,
        ] {
            for risk in [RiskLevel::Low, RiskLevel::Medium, RiskLevel::High] {
                let _ = gate.confirm(op, risk, required_phrase(risk));
            }
        }

        self.core.gate = gate.clone();
        self.install_output.clear();
        self.plan_log.clear();
        self.phase_progress = 0;
        self.screen = Screen::Progress;

        let mut runner = RealSystemRunner::new(gate.clone());
        let mut sink = AppSink { app: self };
        let logger =
            camarelol_core::logging::Logger::new(std::path::Path::new("/var/log/camarelol"));
        let report = run_install(&cfg, &device, &plan, &gate, &logger, &mut runner, &mut sink);
        let completed = report.all_completed();
        self.outcome = Some(match report.outcome {
            Outcome::Completed => Outcome::Completed,
            Outcome::CompletedWithWarnings => Outcome::CompletedWithWarnings,
            _ => Outcome::Incomplete,
        });
        self.install_output.push(String::new());
        self.install_output.push(report.render());
        if completed {
            self.install_output
                .push("\ncamareLOL finished the installation phases.".to_string());
        } else {
            self.install_output.push(format!(
                "\nThe installation stopped early. See {} for details.",
                "/var/log/camarelol"
            ));
            self.log_path = "/var/log/camarelol".into();
        }
        if let Some(p) = &self.save_config_path {
            let _ = cfg.save(p.to_str().unwrap_or_default());
        }
        self.report = Some(report);
        self.screen = Screen::Done;
    }

    /// Report of the install for the Done screen.
    pub fn outcome_text(&self) -> String {
        match self.outcome {
            Some(Outcome::Completed) => self
                .tr(
                    "Hoàn tất. Bạn có thể khởi động lại và rút USB cài đặt.",
                    "Completed. You may reboot and remove the installation medium.",
                )
                .to_string(),
            Some(Outcome::CompletedWithWarnings) => self
                .tr(
                    "Hoàn tất nhưng có cảnh báo — hãy đọc nhật ký.",
                    "Completed with warnings — please read the log.",
                )
                .to_string(),
            Some(_) => self
                .tr(
                    "Chưa hoàn tất. Hãy xem nhật ký và phần cứu hộ.",
                    "Not finished. See the log and the recovery section.",
                )
                .to_string(),
            None => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;

    fn app() -> App {
        let mut app = App::new(None);
        app.armed = true;
        app
    }

    #[test]
    fn enter_advances_on_every_step_that_collects_choices() {
        // A user pressing Enter repeatedly must never get stuck.
        let mut app = app();

        app.screen = Screen::Layout;
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            app.screen,
            Screen::Filesystem,
            "Enter must leave the layout step"
        );

        app.screen = Screen::Packages;
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(
            app.screen,
            Screen::Review,
            "Enter must leave the packages step"
        );
    }

    #[test]
    fn f1_toggles_the_help_overlay() {
        let mut app = app();
        app.screen = Screen::Timezone;

        app.handle_key(KeyEvent::from(KeyCode::F(1)));
        assert_eq!(app.screen, Screen::Help);

        // The same key must close it again, not just Esc.
        app.handle_key(KeyEvent::from(KeyCode::F(1)));
        assert_eq!(
            app.screen,
            Screen::Timezone,
            "F1 must close the help overlay"
        );
    }

    #[test]
    fn esc_leaves_the_help_overlay() {
        let mut app = app();
        app.screen = Screen::Timezone;
        app.handle_key(KeyEvent::from(KeyCode::F(1)));
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(app.screen, Screen::Timezone);
    }

    /// A storage screen with two empty disks.
    ///
    /// Each call gets its own survey file. The suite runs in parallel, and two
    /// tests sharing one file would race: whichever read landed while the other
    /// was writing saw a truncated document and got an empty device list.
    fn storage_fixture() -> App {
        let survey = serde_json::json!([
            {
                "kernel_name": "sda", "path": "/dev/sda", "size_bytes": 21474836480i64,
                "rotational": false, "removable": false, "kind": "SataSsd",
                "partition_table": "None", "partitions": [], "mount_state": "NotMounted",
                "os_hints": [], "encrypted": null, "risk": "Low"
            },
            {
                "kernel_name": "sdb", "path": "/dev/sdb", "size_bytes": 21474836480i64,
                "rotational": false, "removable": false, "kind": "SataSsd",
                "partition_table": "None", "partitions": [], "mount_state": "NotMounted",
                "os_hints": [], "encrypted": null, "risk": "Low"
            }
        ]);
        let dir = std::env::temp_dir().join("camarelol-tui-tests");
        std::fs::create_dir_all(&dir).unwrap();
        static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = dir.join(format!("survey-{n}.json"));
        std::fs::write(&path, survey.to_string()).unwrap();

        let mut app = App::new(Some(path.clone()));
        // Pin the firmware so the wizard behaves the same on any host.
        app.core
            .set_firmware(camarelol_core::hardware::Firmware::Uefi);
        app.armed = true;
        // Fail here rather than in the test body: an unreadable survey shows up
        // as a confusing "expected 2 devices, got 0" further down.
        assert_eq!(
            app.devices().len(),
            2,
            "the fixture survey at {} did not load",
            path.display()
        );
        app
    }

    /// Put the wizard on the accounts screen the way the user reaches it.
    fn accounts_fixture() -> App {
        let mut app = storage_fixture();
        app.screen = Screen::Filesystem;
        app.go_next();
        assert_eq!(app.screen, Screen::Accounts);
        app
    }

    #[test]
    fn typing_lands_in_the_first_field_without_pressing_tab_first() {
        // Regression: the form arrived with no field focused, so every
        // keystroke was dropped and the screen looked frozen.
        let mut app = accounts_fixture();
        assert_eq!(app.field, Field::Hostname);
        let before = app.hostname.clone();
        for c in "pc".chars() {
            app.handle_key(KeyEvent::from(KeyCode::Char(c)));
        }
        assert_eq!(app.hostname, format!("{before}pc"));
    }

    #[test]
    fn arriving_by_esc_also_focuses_the_first_field() {
        // Walking back into the form used to leave the cursor nowhere, which is
        // the same "typing does nothing" trap as arriving for the first time.
        let mut app = accounts_fixture();
        app.screen = Screen::Bootloader;
        app.handle_key(KeyEvent::from(KeyCode::Esc));
        assert_eq!(app.screen, Screen::Accounts);
        assert_eq!(app.field, Field::Hostname);

        app.hostname.clear();
        app.handle_key(KeyEvent::from(KeyCode::Char('x')));
        assert_eq!(app.hostname, "x");
    }

    #[test]
    fn arrow_keys_move_between_account_fields() {
        let mut app = accounts_fixture();
        assert_eq!(app.field, Field::Hostname);

        app.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(app.field, Field::RootPassword);
        app.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(app.field, Field::RootPasswordConfirm);

        app.handle_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(app.field, Field::RootPassword);
    }

    #[test]
    fn tab_and_the_arrows_visit_the_same_fields() {
        let by_tab = {
            let mut app = accounts_fixture();
            let mut seen = vec![app.field];
            for _ in 0..6 {
                app.handle_key(KeyEvent::from(KeyCode::Tab));
                seen.push(app.field);
            }
            seen
        };
        let by_arrow = {
            let mut app = accounts_fixture();
            let mut seen = vec![app.field];
            for _ in 0..6 {
                app.handle_key(KeyEvent::from(KeyCode::Down));
                seen.push(app.field);
            }
            seen
        };
        assert_eq!(by_tab, by_arrow);
        assert_eq!(
            by_tab.len(),
            7,
            "all six fields plus the admin row are reachable"
        );
    }

    #[test]
    fn the_admin_row_is_reachable_and_toggles() {
        // It used to be a feature with no way to see it: nothing drew the
        // state, and only a Space press in an unfocused screen changed it.
        let mut app = accounts_fixture();
        app.user_is_admin = true;

        for _ in 0..6 {
            app.handle_key(KeyEvent::from(KeyCode::Down));
        }
        assert_eq!(app.field, Field::Admin);

        app.handle_key(KeyEvent::from(KeyCode::Char(' ')));
        assert!(!app.user_is_admin, "Space must toggle the admin row");
        app.handle_key(KeyEvent::from(KeyCode::Right));
        assert!(app.user_is_admin, "Right must toggle it back");
    }

    #[test]
    fn the_admin_flag_reaches_the_built_config() {
        let mut app = accounts_fixture();
        for _ in 0..6 {
            app.handle_key(KeyEvent::from(KeyCode::Down));
        }
        app.handle_key(KeyEvent::from(KeyCode::Char(' ')));

        // Admin sits below the password fields; the username is the third
        // one up from it (Admin, UserPasswordConfirm, UserPassword, Username).
        for _ in 0..3 {
            app.handle_key(KeyEvent::from(KeyCode::Up));
        }
        assert_eq!(app.field, Field::Username);
        for c in "alice".chars() {
            app.handle_key(KeyEvent::from(KeyCode::Char(c)));
        }

        app.user_is_admin = false;
        let without = app.build_config().unwrap();
        assert!(
            without.admin_users.is_empty(),
            "a non-admin user must not be in admin_users"
        );

        app.user_is_admin = true;
        assert_eq!(app.build_config().unwrap().admin_users, vec!["alice"]);
    }

    #[test]
    fn esc_walks_back_one_step_on_every_wizard_screen() {
        // The footer offers Esc everywhere, but six screens used to drop it,
        // turning them into one-way ratchets.
        for screen in Screen::ORDER {
            if !matches!(screen, Screen::Welcome | Screen::Progress | Screen::Done) {
                let mut app = storage_fixture();
                app.screen = screen;
                app.handle_key(KeyEvent::from(KeyCode::Esc));
                assert_eq!(
                    app.screen,
                    Screen::at(screen.index() - 1),
                    "Esc on {:?} must land on the previous step",
                    screen
                );
            }
        }
    }

    #[test]
    fn esc_does_nothing_where_there_is_nowhere_to_go() {
        for screen in [Screen::Welcome, Screen::Progress, Screen::Done] {
            let mut app = storage_fixture();
            app.screen = screen;
            app.handle_key(KeyEvent::from(KeyCode::Esc));
            assert_eq!(app.screen, screen, "Esc on {:?} must be inert", screen);
        }
    }

    #[test]
    fn arrow_keys_move_the_disk_cursor_on_the_storage_screen() {
        // Regression: the storage screen fell through to a default arm that
        // moved the *language* selection, so the arrows did nothing.
        let mut app = storage_fixture();
        assert_eq!(app.devices().len(), 2);
        app.screen = Screen::Storage;
        assert_eq!(app.storage_index, 0);

        app.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(app.storage_index, 1, "Down must select the second disk");

        app.handle_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(app.storage_index, 0, "Up must go back to the first disk");
    }

    #[test]
    fn the_disk_cursor_stays_inside_the_list() {
        let mut app = storage_fixture();
        app.screen = Screen::Storage;

        app.handle_key(KeyEvent::from(KeyCode::Up));
        assert_eq!(app.storage_index, 0, "must not wrap below the first disk");

        app.handle_key(KeyEvent::from(KeyCode::Down));
        app.handle_key(KeyEvent::from(KeyCode::Down));
        app.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(app.storage_index, 1, "must not run past the last disk");
    }

    #[test]
    fn enter_picks_the_disk_under_the_cursor() {
        let mut app = storage_fixture();
        app.screen = Screen::Storage;
        app.handle_key(KeyEvent::from(KeyCode::Down));
        app.handle_key(KeyEvent::from(KeyCode::Enter));
        assert_eq!(app.selected_disk.as_deref(), Some("/dev/sdb"));
        assert_eq!(app.screen, Screen::DiskMode);
    }

    #[test]
    fn arrow_keys_do_not_disturb_other_screens() {
        let mut app = storage_fixture();
        app.screen = Screen::Timezone;
        app.language_index = 1;
        let before = app.timezone_index;
        app.handle_key(KeyEvent::from(KeyCode::Down));
        assert_eq!(app.timezone_index, before + 1);
        assert_eq!(
            app.language_index, 1,
            "moving the timezone must not touch the language"
        );
    }

    #[test]
    fn no_step_is_a_dead_end_for_enter() {
        // Walk the whole wizard with Enter only and make sure it terminates.
        let mut app = app();
        app.screen = Screen::Welcome;
        for _ in 0..40 {
            if matches!(app.screen, Screen::Review | Screen::Progress | Screen::Done) {
                break;
            }
            let before = app.screen;
            app.handle_key(KeyEvent::from(KeyCode::Enter));
            assert_ne!(before, app.screen, "Enter does nothing on {before:?}");
        }
        assert!(
            matches!(app.screen, Screen::Review | Screen::Progress | Screen::Done),
            "wizard did not reach the review screen, stuck at {:?}",
            app.screen
        );
    }
}
