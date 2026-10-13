//! Rendering for the camareLOL TUI. Keyboard-first, works on small terminals.

use super::app::{App, Field, Screen};
use camarelol_core::desktop::{DesktopProfile, DisplayServer};
use camarelol_core::locale::{KEYMAPS, TIMEZONES};
use camarelol_core::network;
use camarelol_core::safety::{required_phrase, GateMode};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Gauge, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

const ACCENT: Color = Color::Cyan;
const WARN: Color = Color::Yellow;
const BAD: Color = Color::Red;
const GOOD: Color = Color::Green;

pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();
    if size.height < 8 || size.width < 30 {
        let p = Paragraph::new("Terminal too small").style(Style::default().fg(BAD));
        f.render_widget(p, size);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(2),
        ])
        .split(size);

    draw_header(f, app, chunks[0]);

    match app.screen {
        Screen::Help => draw_help(f, app, chunks[1]),
        Screen::Logs => draw_logs(f, app, chunks[1]),
        other => match other {
            Screen::Welcome => draw_welcome(f, app, chunks[1]),
            Screen::Language
            | Screen::Keyboard
            | Screen::Timezone
            | Screen::Filesystem
            | Screen::Bootloader => draw_choice(f, app, chunks[1]),
            Screen::Storage => draw_storage(f, app, chunks[1]),
            Screen::DiskMode => draw_disk_mode(f, app, chunks[1]),
            Screen::Layout => draw_layout(f, app, chunks[1]),
            Screen::Accounts => draw_accounts(f, app, chunks[1]),
            Screen::Desktop => draw_desktop(f, app, chunks[1]),
            Screen::Services => draw_services(f, app, chunks[1]),
            Screen::Packages => draw_packages(f, app, chunks[1]),
            Screen::Review => draw_review(f, app, chunks[1]),
            Screen::Progress => draw_progress(f, app, chunks[1]),
            Screen::Done => draw_done(f, app, chunks[1]),
            _ => {}
        },
    }

    draw_footer(f, app, chunks[2]);

    if let Some(msg) = app.message.clone() {
        let area = centered_rect(70, 5, size);
        f.render_widget(Clear, area);
        let p = Paragraph::new(msg)
            .block(Block::default().borders(Borders::ALL).title(" Info "))
            .wrap(Wrap { trim: true });
        f.render_widget(p, area);
    }
    if let Some(err) = app.error.clone() {
        let detail = format!(
            "{}\n\n{}",
            err.message,
            err.remediation.clone().unwrap_or_default()
        );
        let area = centered_rect(78, 8, size);
        f.render_widget(Clear, area);
        let p = Paragraph::new(detail)
            .style(Style::default().fg(BAD))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" Error — {} ", err.code)),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(p, area);
    }
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let w = area.width * percent_x / 100;
    let x = (area.width - w) / 2;
    let y = (area.height - height.min(area.height)) / 2;
    Rect {
        x: area.x + x,
        y: area.y + y,
        width: w,
        height: height.min(area.height),
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let step = if matches!(app.screen, Screen::Help | Screen::Logs) {
        0
    } else {
        app.screen.index() + 1
    };
    let total = Screen::ORDER.len();
    let title = format!(
        " camareLOL v{} — {}  ({}/{}) ",
        env!("CARGO_PKG_VERSION"),
        app.screen.title(),
        step.min(total),
        total
    );
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(ACCENT).bg(Color::DarkGray))
        .ratio((step as f64 / total as f64).min(1.0))
        .label(Span::styled(
            title,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
    f.render_widget(gauge, inner);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let gate = app.gate();
    let mode = match gate.mode() {
        GateMode::Locked => "locked",
        GateMode::DryRun => "dry-run",
        GateMode::Execute => "EXECUTE",
    };
    let mode_text = mode.to_string();
    let vi = format!(
        " ↑↓ di chuyển · Enter chọn · Tab đổi trường · Space bật/tắt · Esc quay lại · F1 trợ giúp · F10 xem lại · F5 quét lại | cổng an toàn: {mode_text}"
    );
    let en = format!(
        " ↑↓ move · Enter select · Tab switch field · Space toggle · Esc back · F1 help · F10 review · F5 rescan | safety gate: {mode_text}"
    );
    let text = app.tr_owned(vi, en);
    let style = if matches!(gate.mode(), GateMode::Locked) {
        Style::default().fg(WARN)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    f.render_widget(Paragraph::new(text).style(style), area);
}

fn screen_block(title: impl Into<String>) -> Block<'static> {
    let title: String = title.into();
    Block::default().borders(Borders::ALL).title(Span::styled(
        format!(" {title} "),
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    ))
}

// ------------------------------------------------------------------ screens

fn draw_welcome(f: &mut Frame, app: &App, area: Rect) {
    let hw = camarelol_core::hardware::summarize();
    let text = vec![
        Line::from(vec![Span::styled(
            "  c a m a r e L O L",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )]),
        Line::from("  A guided, safety-first Arch Linux installer for the terminal"),
        Line::from(""),
        Line::from(vec![
            Span::raw("  Firmware:   "),
            Span::styled(hw.firmware.as_str(), Style::default().fg(GOOD)),
        ]),
        Line::from(format!("  CPU:        {}", hw.cpu.model)),
        Line::from(format!(
            "  Memory:     {}",
            camarelol_core::storage::human_size(hw.ram_bytes)
        )),
        Line::from(format!(
            "  Network:    {}",
            network::describe(app.connectivity)
        )),
        Line::from(format!(
            "  Disks:      {} detected",
            app.devices().len()
        )),
        Line::from(""),
        Line::from(app.tr(
            "  Nhấn Enter để bắt đầu. Không có thao tác phá hủy nào diễn ra trước khi bạn xác nhận.",
            "  Press Enter to start. Nothing destructive happens before you confirm it.",
        )),
    ];
    f.render_widget(
        Paragraph::new(text)
            .block(screen_block("Welcome"))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_choice(f: &mut Frame, app: &mut App, area: Rect) {
    let (items, selected): (Vec<String>, usize) = match app.screen {
        Screen::Language => (
            vec!["Tiếng Việt".to_string(), "English".to_string()],
            app.language_index,
        ),
        Screen::Keyboard => (
            KEYMAPS.iter().map(|s| s.to_string()).collect(),
            app.keymap_index,
        ),
        Screen::Timezone => (
            TIMEZONES.iter().map(|s| s.to_string()).collect(),
            app.timezone_index,
        ),
        Screen::Filesystem => (
            vec![
                "ext4 — recommended, journaling".to_string(),
                "btrfs — snapshots and subvolumes".to_string(),
                "xfs — separate /boot required with GRUB".to_string(),
            ],
            app.fs_index,
        ),
        Screen::Bootloader => (
            vec!["GRUB".to_string(), "systemd-boot".to_string()],
            app.bootloader_index,
        ),
        _ => (vec![], 0),
    };
    let rows: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let style = if i == selected {
                Style::default().fg(Color::Black).bg(ACCENT)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![Span::styled(format!(" {s}"), style)]))
        })
        .collect();
    let mut state = ListState::default();
    state.select(Some(selected));
    let list = List::new(rows).block(screen_block(app.screen.title()));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_storage(f: &mut Frame, app: &mut App, area: Rect) {
    let devices = app.devices().to_vec();
    let items: Vec<ListItem> = devices
        .iter()
        .map(|d| {
            let risk_color = match d.risk {
                camarelol_core::storage::RiskLevel::Low => GOOD,
                camarelol_core::storage::RiskLevel::Medium => WARN,
                camarelol_core::storage::RiskLevel::High => BAD,
            };
            let mut lines = vec![
                Line::from(vec![
                    Span::styled(
                        format!("▸ {}", d.summary_line()),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("   "),
                    Span::styled(d.risk.as_str(), Style::default().fg(risk_color)),
                ]),
                Line::from(format!("    {}", d.path)),
            ];
            if let Some(model) = &d.model {
                lines.push(Line::from(format!("    {model}")));
            }
            if d.has_windows() {
                lines.push(Line::from(vec![Span::styled(
                    "    another operating system detected",
                    Style::default().fg(BAD),
                )]));
            }
            if d.used_by_live_system() {
                lines.push(Line::from(vec![Span::styled(
                    "    in use by the live system",
                    Style::default().fg(BAD),
                )]));
            }
            ListItem::new(lines)
        })
        .collect();
    let mut state = ListState::default();
    state.select(Some(app.storage_index));
    let list = List::new(items).block(screen_block(
        "Storage devices (↑↓ to choose, Enter to continue)",
    ));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_disk_mode(f: &mut Frame, app: &mut App, area: Rect) {
    let modes = app.disk_modes();
    let mut items: Vec<ListItem> = modes
        .iter()
        .map(|m| ListItem::new(Line::from(format!(" {m}"))))
        .collect();
    if let Some(dev) = app.device(app.selected_disk.as_deref().unwrap_or("")) {
        items.push(ListItem::new(Line::from("")));
        items.push(ListItem::new(Line::from(format!(
            "  {} — {} — {}",
            dev.path,
            dev.kind.as_str(),
            dev.capacity_display()
        ))));
    }
    let mut state = ListState::default();
    state.select(Some(app.disk_mode_index));
    let list = List::new(items).block(screen_block(
        app.tr("Chọn chế độ sử dụng ổ đĩa", "Choose how this disk is used"),
    ));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_layout(f: &mut Frame, app: &mut App, area: Rect) {
    let choices = [
        "Root partition only",
        "Root + /home",
        "Root + /boot",
        "Root + /home + /boot",
    ];
    let mut lines: Vec<Line> = choices
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let marker = if i == app.layout_index { "●" } else { "○" };
            Line::from(format!(" {marker} {c}"))
        })
        .collect();
    lines.push(Line::from(""));
    if let Some(plan) = &app.plan {
        for l in plan.render().lines() {
            lines.push(Line::from(format!(" {l}")));
        }
    }
    let p = Paragraph::new(lines)
        .block(screen_block(app.tr("Bố cục phân vùng", "Partition layout")))
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn draw_accounts(f: &mut Frame, app: &mut App, area: Rect) {
    let field_style = |field: Field| {
        if app.field == field {
            Style::default().fg(Color::Black).bg(ACCENT)
        } else {
            Style::default()
        }
    };
    let pw = |s: &str| "*".repeat(s.chars().count());
    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  Hostname:        "),
            Span::styled(app.hostname.clone(), field_style(Field::Hostname)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  Root password:   "),
            Span::styled(pw(&app.root_password), field_style(Field::RootPassword)),
        ]),
        Line::from(vec![
            Span::raw("  Repeat:          "),
            Span::styled(
                pw(&app.root_password_confirm),
                field_style(Field::RootPasswordConfirm),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("  User name:       "),
            Span::styled(app.username.clone(), field_style(Field::Username)),
        ]),
        Line::from(vec![
            Span::raw("  User password:   "),
            Span::styled(pw(&app.user_password), field_style(Field::UserPassword)),
        ]),
        Line::from(vec![
            Span::raw("  Repeat:          "),
            Span::styled(
                pw(&app.user_password_confirm),
                field_style(Field::UserPasswordConfirm),
            ),
        ]),
        Line::from(""),
        Line::from(app.tr(
            "  Mật khẩu chỉ giữ trong RAM: không bao giờ ghi vào tệp cấu hình hay nhật ký.",
            "  Passwords live in RAM only: they are never written to config files or logs.",
        )),
    ];
    let p = Paragraph::new(lines)
        .block(screen_block(app.tr("Tài khoản", "Accounts")))
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn draw_desktop(f: &mut Frame, app: &mut App, area: Rect) {
    let mut lines: Vec<Line> = vec![Line::from(app.tr("  Chọn desktop:", "  Choose a desktop:"))];
    for (i, p) in DesktopProfile::ALL.iter().enumerate() {
        let marker = if i == app.desktop_index { "●" } else { "○" };
        lines.push(Line::from(format!("   {marker} {}", p.as_str())));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(
        app.tr("  Chế độ hiển thị (→/Tab):", "  Display server (→/Tab):"),
    ));
    for (i, s) in DisplayServer::ALL.iter().enumerate() {
        let marker = if i == app.server_index { "●" } else { "○" };
        lines.push(Line::from(format!("   {marker} {}", s.as_str())));
    }
    if let Some(dm) = app.display_manager() {
        lines.push(Line::from(format!("   display manager: {dm}")));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(app.tr(
        "  ⚠ Sway/Hyprland chỉ chạy Wayland.",
        "  ⚠ Sway/Hyprland are Wayland-only.",
    )));
    let p = Paragraph::new(lines)
        .block(screen_block(
            app.tr("Môi trường desktop", "Desktop environment"),
        ))
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn draw_services(f: &mut Frame, app: &mut App, area: Rect) {
    let marker = |on: bool| if on { "[x]" } else { "[ ]" };
    let audio = match app.services.audio {
        camarelol_core::services::AudioStack::None => "none",
        camarelol_core::services::AudioStack::PipeWire => "pipewire",
        camarelol_core::services::AudioStack::PulseAudio => "pulseaudio",
    };
    let entries = [
        format!("{} NetworkManager", marker(app.services.network_manager)),
        format!("{} Bluetooth", marker(app.services.bluetooth)),
        format!("{} Audio: {audio}", marker(true)),
        format!("{} zram", marker(app.services.zram)),
        format!("{} Printing (cups)", marker(app.services.printing)),
        format!("{} SSH server", marker(app.services.ssh)),
    ];
    let items: Vec<ListItem> = entries
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let style = if i == app.services_cached_index {
                Style::default().fg(Color::Black).bg(ACCENT)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![Span::styled(format!(" {s}"), style)]))
        })
        .collect();
    let mut state = ListState::default();
    state.select(Some(app.services_cached_index));
    let list = List::new(items).block(screen_block(app.tr("Dịch vụ", "Services")));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_packages(f: &mut Frame, app: &mut App, area: Rect) {
    let suggestions = app.package_suggestions();
    let items: Vec<ListItem> = suggestions
        .iter()
        .enumerate()
        .map(|(i, (name, desc))| {
            let on = app.custom_packages.contains(&name.to_string());
            let checkbox = if on { "[x]" } else { "[ ]" };
            let style = if i == app.package_index {
                Style::default().fg(Color::Black).bg(ACCENT)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![Span::styled(
                format!(" {checkbox} {name:<12} {desc}"),
                style,
            )]))
        })
        .collect();
    let mut state = ListState::default();
    state.select(Some(app.package_index));
    let block = screen_block(app.tr("Gói bổ sung", "Additional packages"));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(inner);
    let list = List::new(items);
    f.render_stateful_widget(list, chunks[0], &mut state);
    let input = Paragraph::new(app.custom_package_input.clone()).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" add package (Tab) "),
    );
    f.render_widget(input, chunks[1]);
}

fn draw_review(f: &mut Frame, app: &mut App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();
    if let Some(plan) = &app.plan {
        for l in plan.render().lines() {
            lines.push(Line::from(format!(" {l}")));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            " Destructive operations:",
            Style::default().fg(WARN),
        )]));
        for op in plan.destructive_operations() {
            lines.push(Line::from(format!("   • {op}")));
        }
        lines.push(Line::from(""));
        let phrase = required_phrase(plan.disk_risk);
        lines.push(Line::from(vec![
            Span::raw(" Type "),
            Span::styled(
                phrase,
                Style::default().fg(BAD).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" to confirm: "),
            Span::styled(
                app.confirm_phrase.clone(),
                Style::default().fg(Color::Black).bg(Color::White),
            ),
        ]));
    } else {
        lines.push(Line::from(" No plan yet — open the layout step first."));
    }
    let p = Paragraph::new(lines)
        .block(screen_block(app.tr(
            "Xem lại kế hoạch (Enter để cài)",
            "Review plan (Enter to install)",
        )))
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn draw_progress(f: &mut Frame, app: &mut App, area: Rect) {
    let total = camarelol_core::install::PHASE_ORDER.len() as f64;
    let ratio = (app.phase_progress as f64 / total.max(1.0)).min(1.0);
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(GOOD))
        .ratio(ratio)
        .label(format!("{} / {}", app.phase_progress, total as usize));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(3)])
        .split(area);
    f.render_widget(gauge, chunks[0]);
    let items: Vec<ListItem> = app
        .install_output
        .iter()
        .map(|l| ListItem::new(Line::from(l.clone())))
        .collect();
    let list = List::new(items).block(screen_block("Progress"));
    f.render_widget(list, chunks[1]);
}

fn draw_done(f: &mut Frame, app: &mut App, area: Rect) {
    let outcome_color = match app.outcome {
        Some(camarelol_core::install::Outcome::Completed) => GOOD,
        Some(camarelol_core::install::Outcome::CompletedWithWarnings) => WARN,
        _ => BAD,
    };
    let mut lines = vec![
        Line::from(Span::styled(
            app.outcome_text(),
            Style::default()
                .fg(outcome_color)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    if let Some(report) = &app.report {
        for p in &report.phases {
            lines.push(Line::from(format!(
                "  {:<14} {:<10} {}",
                p.name, p.state, p.details
            )));
        }
    }
    let p = Paragraph::new(lines)
        .block(screen_block("Result"))
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn draw_help(f: &mut Frame, _app: &App, area: Rect) {
    let lines = vec![
        Line::from(""),
        Line::from("  ↑ / ↓      move / navigate"),
        Line::from("  Enter      select, or start the step"),
        Line::from("  Space      toggle an option"),
        Line::from("  Tab        next field"),
        Line::from("  Esc        back (before anything irreversible)"),
        Line::from("  F1         this help"),
        Line::from("  F5         re-scan hardware and disks"),
        Line::from("  F10        review the plan"),
        Line::from("  Ctrl-C     quit"),
        Line::from(""),
        Line::from("  camareLOL never writes to a disk until you type the confirmation"),
        Line::from("  phrase on the review screen, and it re-checks the disk identity"),
        Line::from("  immediately before every write."),
    ];
    f.render_widget(Paragraph::new(lines).block(screen_block("Help")), area);
}

fn draw_logs(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .install_output
        .iter()
        .map(|l| ListItem::new(Line::from(l.clone())))
        .collect();
    let list = List::new(items)
        .block(screen_block("Installation log (Esc to close)"))
        .style(Style::default());
    f.render_widget(list, area);
}
