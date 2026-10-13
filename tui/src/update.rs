//! Key handling / navigation for the camareLOL TUI.

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
            KeyCode::F(1) => {
                self.previous = Some(self.screen);
                self.screen = Screen::Help;
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
                KeyCode::Esc => self.screen = Screen::Storage,
                _ => {}
            },
            Screen::Layout => match key.code {
                KeyCode::Up => self.layout_index = self.layout_index.saturating_sub(1),
                KeyCode::Down => self.layout_index = (self.layout_index + 1).min(3),
                KeyCode::Enter => self.rebuild_plan(),
                KeyCode::Right => {
                    self.rebuild_plan();
                    self.go_next();
                }
                KeyCode::Esc => self.screen = Screen::DiskMode,
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
                KeyCode::Right => self.go_next(),
                _ => {}
            },
            Screen::Accounts => self.handle_accounts_key(key),
            Screen::Review => match key.code {
                KeyCode::Enter => self.try_start_install(),
                KeyCode::Esc => self.screen = Screen::Packages,
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
    }

    fn move_cursor(&mut self, delta: i32) {
        let count = match self.screen {
            Screen::Language => 2,
            Screen::Keyboard => camarelol_core::locale::KEYMAPS.len(),
            Screen::Timezone => camarelol_core::locale::TIMEZONES.len(),
            Screen::Filesystem => 3,
            Screen::Bootloader => 2,
            _ => 1,
        };
        let current = match self.screen {
            Screen::Language => &mut self.language_index,
            Screen::Keyboard => &mut self.keymap_index,
            Screen::Timezone => &mut self.timezone_index,
            Screen::Filesystem => &mut self.fs_index,
            Screen::Bootloader => &mut self.bootloader_index,
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

    fn handle_accounts_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Tab => {
                self.field = match self.field {
                    Field::None => Field::Hostname,
                    Field::Hostname => Field::RootPassword,
                    Field::RootPassword => Field::RootPasswordConfirm,
                    Field::RootPasswordConfirm => Field::Username,
                    Field::Username => Field::UserPassword,
                    Field::UserPassword => Field::UserPasswordConfirm,
                    Field::UserPasswordConfirm => Field::None,
                    _ => Field::None,
                };
            }
            KeyCode::Char(' ') if self.field == Field::None => {
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
