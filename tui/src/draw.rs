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

/// Style used for the selected row of a list. Without it ratatui renders the
/// selection identically to everything else, and the cursor is invisible.
fn selected_row() -> Style {
    Style::default()
        .bg(ACCENT)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD)
}
const WARN: Color = Color::Yellow;
const BAD: Color = Color::Red;
const GOOD: Color = Color::Green;

/// The one line that must be on screen in preview mode, on every screen.
///
/// Not decoration: a user who believes a preview will erase a disk behaves
/// completely differently from one who does not, and the difference matters
/// exactly once — when it is too late. Kept in both languages because the
/// installer is bilingual and the warning is the one string that must never be
/// missed.
pub const PREVIEW_BANNER: &str = "PREVIEW — KHÔNG GÌ ĐƯỢC GHI VÀO ĐĨA";

/// The same warning in Simplified Chinese, worded as `docs/zh/index.md` words
/// it. Kept separate from [`PREVIEW_BANNER`] because that constant is asserted
/// on verbatim by the acceptance tests in three languages; merging them would
/// change a string the suite depends on.
pub const PREVIEW_BANNER_ZH: &str = "预览 — 不会向磁盘写入任何内容";

fn preview_banner(app: &App) -> String {
    app.tr_owned(
        format!(" ⚠ {PREVIEW_BANNER} · lệnh chỉ được in ra, không lệnh nào chạy "),
        format!(" ⚠ {PREVIEW_BANNER} — commands are shown, none is run "),
        format!(" ⚠ {PREVIEW_BANNER_ZH} — 只显示命令，不会执行任何命令 "),
    )
}

pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();
    if size.height < 9 || size.width < 30 {
        let p = Paragraph::new("Terminal too small").style(Style::default().fg(BAD));
        f.render_widget(p, size);
        return;
    }

    // The banner row only exists in preview. `app.preview` is decided once, from
    // the command line, and never changes — so the layout cannot shift under
    // the user and the row cannot disappear halfway through the wizard.
    let (header, body, banner, footer) = if app.preview {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(1),
                Constraint::Length(2),
            ])
            .split(size);
        (chunks[0], chunks[1], chunks[2], chunks[3])
    } else {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(2),
            ])
            .split(size);
        (chunks[0], chunks[1], Rect::default(), chunks[2])
    };

    draw_header(f, app, header);

    match app.screen {
        Screen::Help => draw_help(f, app, body),
        Screen::Logs => draw_logs(f, app, body),
        other => match other {
            Screen::Welcome => draw_welcome(f, app, body),
            Screen::Language
            | Screen::Keyboard
            | Screen::Timezone
            | Screen::Filesystem
            | Screen::Bootloader => draw_choice(f, app, body),
            Screen::Storage => draw_storage(f, app, body),
            Screen::DiskMode => draw_disk_mode(f, app, body),
            Screen::Layout => draw_layout(f, app, body),
            Screen::Accounts => draw_accounts(f, app, body),
            Screen::Desktop => draw_desktop(f, app, body),
            Screen::Services => draw_services(f, app, body),
            Screen::Packages => draw_packages(f, app, body),
            Screen::Review => draw_review(f, app, body),
            Screen::Progress => draw_progress(f, app, body),
            Screen::Done => draw_done(f, app, body),
            _ => {}
        },
    }

    if app.preview {
        f.render_widget(
            Paragraph::new(preview_banner(app)).style(
                Style::default()
                    .fg(BAD)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            banner,
        );
    }

    draw_footer(f, app, footer);

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
    let preview = app.preview;
    let step = if matches!(app.screen, Screen::Help | Screen::Logs) {
        0
    } else {
        app.screen.index() + 1
    };
    let total = Screen::ORDER.len();
    // Show the commit too: two builds of the same version string are otherwise
    // indistinguishable, and the user has no other way to tell which one they
    // are trusting with a disk.
    let build = crate::version::git_sha();
    let build = if build == "unknown" {
        String::new()
    } else {
        format!("+{build}")
    };
    let title = format!(
        " camareLOL v{}{build} — {}  ({}/{}) ",
        crate::version::VERSION,
        app.screen.title(),
        step.min(total),
        total
    );
    // The header turns red in preview so the state is visible even where the
    // banner row is covered by a dialog.
    let accent = if preview { BAD } else { ACCENT };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(accent).bg(Color::DarkGray))
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
        GateMode::Preview => "PREVIEW",
        GateMode::Execute => "EXECUTE",
    };
    let mode_text = mode.to_string();
    let vi = format!(
        " ↑↓ di chuyển · Enter chọn · Tab đổi trường · Space bật/tắt · / tìm gói · L nạp cấu hình · Esc quay lại · F1 trợ giúp · F10 xem lại · F5 quét lại | cổng an toàn: {mode_text}"
    );
    let en = format!(
        " ↑↓ move · Enter select · Tab switch field · Space toggle · / search packages · L load profile · Esc back · F1 help · F10 review · F5 rescan | safety gate: {mode_text}"
    );
    let zh = format!(
        " ↑↓ 移动 · Enter 选择 · Tab 切换字段 · Space 切换 · / 搜索软件包 · L 加载配置 · Esc 返回 · F1 帮助 · F10 复核 · F5 重新扫描 | 安全闸门：{mode_text}"
    );
    let text = app.tr_owned(vi, en, zh);
    let style = if matches!(gate.mode(), GateMode::Locked | GateMode::Preview) {
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
    let host = &app.host;
    let mut text = vec![
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
        // Only one row for the host, and none at all when it is unsupported:
        // the warning below already names it. Twenty-four rows is a legal
        // terminal and this screen has to fit in one — the update line is the
        // last thing on it and must never be the thing that scrolls away.
        Line::from(format!("  CPU:        {}", hw.cpu.model)),
        Line::from(format!(
            "  Memory:     {}",
            camarelol_core::storage::human_size(hw.ram_bytes)
        )),
        Line::from(format!(
            "  Network:    {}",
            network::describe(app.connectivity)
        )),
        Line::from(format!("  Disks:      {} detected", app.devices().len())),
    ];
    if host.supported {
        text.insert(
            4,
            Line::from(vec![
                Span::raw("  Host:       "),
                Span::styled(host.summary(), Style::default().fg(GOOD)),
            ]),
        );
    }
    // The loaded-profile line and the load prompt share one slot so the screen
    // still fits the smallest terminal this interface supports.
    match (&app.loaded_from, app.field == Field::LoadPath) {
        (_, true) => text.push(Line::from(vec![
            Span::styled("  ▸ profile: ", Style::default().fg(WARN)),
            Span::styled(
                app.load_path_input.clone(),
                Style::default().fg(Color::Black).bg(Color::White),
            ),
        ])),
        (Some(path), false) => text.push(Line::from(vec![
            Span::raw("  Profile:    "),
            Span::styled(
                path.display().to_string(),
                Style::default().fg(GOOD).add_modifier(Modifier::BOLD),
            ),
        ])),
        (None, false) => text.push(Line::from(app.tr(
            "  Press L to load a saved profile, or Enter to start fresh.",
            "  Press L to load a saved profile, or Enter to start fresh.",
            "  按 L 载入已保存的配置，或按 Enter 重新开始。",
        ))),
    }
    text.extend([
        Line::from(app.tr(
            "  Nhấn Enter để bắt đầu. Không có thao tác phá hủy nào diễn ra trước khi bạn xác nhận.",
            "  Press Enter to start. Nothing destructive happens before you confirm it.",
            "  按 Enter 开始。在你确认之前，不会执行任何破坏性操作。",
        )),
        Line::from(""),
    ]);
    // Said here, on the first screen, rather than only at the moment the plan is
    // refused: a user who is told at the end of a wizard has already answered
    // every other question, and the answer to this one is "boot the Arch ISO".
    //
    // Only the short form goes on screen. The full [`HostProfile::reason`] is
    // long enough to wrap onto three rows and push the update line off a
    // 24-row terminal — and it is shown in the error dialog when the plan is
    // refused, which is where a user reads it properly.
    if !host.supported {
        text.push(Line::from(Span::styled(
            app.tr(
                format!("  ⚠ Hệ thống không được hỗ trợ: {}", host.summary()),
                format!("  ⚠ Unsupported host: {}", host.summary()),
                format!("  ⚠ 不受支持的主机：{}", host.summary()),
            ),
            Style::default().fg(BAD).add_modifier(Modifier::BOLD),
        )));
        text.push(Line::from(Span::styled(
            app.tr(
                "    → Khởi động từ ISO Arch rồi chạy camareLOL từ đó (F10: lý do).",
                "    → Boot the official Arch live ISO and run camareLOL there (F10: why).",
                "    → 从官方 Arch 实时环境启动并在其中运行 camareLOL（按 F10 查看原因）。",
            ),
            Style::default().fg(WARN),
        )));
    }
    if app.preview {
        // Said here too, because this is where a preview begins and the user
        // may have walked past the startup warning a long time ago.
        text.push(Line::from(Span::styled(
            format!("  ⚠ {PREVIEW_BANNER}"),
            Style::default().fg(BAD).add_modifier(Modifier::BOLD),
        )));
        text.push(Line::from(app.tr(
            "  Cài đặt sẽ được mô phỏng đầy đủ và in ra từng lệnh thật.",
            "  The whole installation is simulated and every real command is printed.",
            "  整个安装过程将被完整模拟，并打印出每一条真实命令。",
        )));
    }
    text.extend([
        Line::from(""),
        Line::from(app.tr(
            "  Cập nhật: chạy lại scripts/camarelol.sh, hoặc tải bản mới ở trang phát hành.",
            "  To update: re-run scripts/camarelol.sh, or download a release from the releases page.",
            "  更新方式：重新运行 scripts/camarelol.sh，或从发布页下载新版本。",
        )),
    ]);
    // Nói thẳng khi Wi-Fi là đường duy nhất mà máy không có công cụ để nối:
    // "no network connection detected" một mình không giúp ai đoán phải làm
    // gì. Hàm `wifi_setup_available()` và trường `wireless` đã tồn tại từ
    // lâu nhưng không ai gọi — cờ này là lần đầu chúng được dùng thật.
    if app.wifi_needs_setup {
        text.insert(
            5,
            Line::from(Span::styled(
                app.tr(
                    "  ⚠ Chỉ có Wi-Fi và chưa nối, mà máy không có iwctl/nmtui để nối.",
                    "  ⚠ Wi-Fi is the only interface, not connected, and no iwctl/nmtui to connect it.",
                    "  ⚠ 只有 Wi-Fi 且尚未连接，而系统没有 iwctl/nmtui 可用于连接。",
                ),
                Style::default().fg(WARN),
            )),
        );
    }
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
            camarelol_core::locale::UI_LANGUAGES
                .iter()
                .map(|l| l.name().to_string())
                .collect(),
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
    let list = List::new(items)
        .block(screen_block(
            "Storage devices (↑↓ to choose, Enter to continue)",
        ))
        .highlight_style(selected_row())
        .highlight_symbol("▸ ");
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
    let list = List::new(items)
        .block(screen_block(app.tr(
            "Chọn chế độ sử dụng ổ đĩa",
            "Choose how this disk is used",
            "选择这块磁盘的使用方式",
        )))
        .highlight_style(selected_row())
        .highlight_symbol("▸ ");
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
        .block(screen_block(app.tr(
            "Bố cục phân vùng",
            "Partition layout",
            "分区布局",
        )))
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
    // A `▸` on the focused row: without it there is no way to tell which field
    // your keystrokes are going into, which reads as "typing does nothing".
    let label = |name: &str, field: Field| {
        let marker = if app.field == field { "▸" } else { " " };
        format!("{marker} {name}")
    };
    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw(format!("  {}", label("Hostname:      ", Field::Hostname))),
            Span::styled(app.hostname.clone(), field_style(Field::Hostname)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw(format!(
                "  {}",
                label("Root password: ", Field::RootPassword)
            )),
            Span::styled(pw(&app.root_password), field_style(Field::RootPassword)),
        ]),
        Line::from(vec![
            Span::raw(format!(
                "  {}",
                label("Repeat:        ", Field::RootPasswordConfirm)
            )),
            Span::styled(
                pw(&app.root_password_confirm),
                field_style(Field::RootPasswordConfirm),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw(format!("  {}", label("User name:     ", Field::Username))),
            Span::styled(app.username.clone(), field_style(Field::Username)),
        ]),
        Line::from(vec![
            Span::raw(format!(
                "  {}",
                label("User password: ", Field::UserPassword)
            )),
            Span::styled(pw(&app.user_password), field_style(Field::UserPassword)),
        ]),
        Line::from(vec![
            Span::raw(format!(
                "  {}",
                label("Repeat:        ", Field::UserPasswordConfirm)
            )),
            Span::styled(
                pw(&app.user_password_confirm),
                field_style(Field::UserPasswordConfirm),
            ),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                label("User is admin: ", Field::Admin),
                field_style(Field::Admin),
            ),
            Span::styled(
                if app.user_is_admin {
                    "yes (sudo)"
                } else {
                    "no"
                },
                field_style(Field::Admin),
            ),
        ]),
        Line::from(""),
        Line::from(app.tr(
            "  Mật khẩu chỉ giữ trong RAM: không bao giờ ghi vào tệp cấu hình hay nhật ký.",
            "  Passwords live in RAM only: they are never written to config files or logs.",
            "  密码只保存在内存中：绝不会写入配置文件或日志。",
        )),
    ];
    let p = Paragraph::new(lines)
        .block(screen_block(app.tr("Tài khoản", "Accounts", "用户账户")))
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn draw_desktop(f: &mut Frame, app: &mut App, area: Rect) {
    let mut lines: Vec<Line> = vec![Line::from(app.tr(
        "  Chọn desktop:",
        "  Choose a desktop:",
        "  选择桌面环境：",
    ))];
    for (i, p) in DesktopProfile::ALL.iter().enumerate() {
        let marker = if i == app.desktop_index { "●" } else { "○" };
        lines.push(Line::from(format!("   {marker} {}", p.as_str())));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(app.tr(
        "  Chế độ hiển thị (→/Tab):",
        "  Display server (→/Tab):",
        "  显示服务器（→/Tab）：",
    )));
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
        "  ⚠ Sway/Hyprland 仅支持 Wayland。",
    )));
    let p = Paragraph::new(lines)
        .block(screen_block(app.tr(
            "Môi trường desktop",
            "Desktop environment",
            "桌面环境",
        )))
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
    let list = List::new(items).block(screen_block(app.tr("Dịch vụ", "Services", "服务")));
    f.render_stateful_widget(list, area, &mut state);
}

fn draw_packages(f: &mut Frame, app: &App, area: Rect) {
    let rows = app.package_rows();
    // Row 0 of the list is the AUR switch, so the cursor index lines up with
    // what is on screen without an extra offset anywhere anywhere else.
    let mut items: Vec<ListItem> = vec![ListItem::new(Line::from(format!(
        " [{}] AUR packages — user-submitted build scripts, never built as root",
        if app.aur_enabled { "x" } else { " " }
    )))];
    for entry in &rows {
        let on = if entry.source == camarelol_core::packages::PackageSource::Aur {
            app.aur_selected.iter().any(|p| p == entry.name)
        } else {
            app.package_is_selected(entry.name)
        };
        let checkbox = if on { "[x]" } else { "[ ]" };
        items.push(ListItem::new(Line::from(format!(
            " {checkbox} {:<22} {:<36} {}",
            entry.name,
            entry.description,
            entry.source.as_str()
        ))));
    }
    if rows.is_empty() {
        items.push(ListItem::new(Line::from(Span::styled(
            format!("   nothing in the catalog matches `{}`", app.package_query),
            Style::default().fg(WARN),
        ))));
    }
    let mut state = ListState::default();
    state.select(Some(app.package_index.min(rows.len())));
    let block = screen_block(app.tr(
        "Gói bổ sung — / để tìm",
        "Additional packages — / to search",
        "附加软件包 — 按 / 搜索",
    ));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
        ])
        .split(inner);
    let list = List::new(items)
        .highlight_style(selected_row())
        .highlight_symbol("▸ ");
    f.render_stateful_widget(list, chunks[0], &mut state);

    let search = Paragraph::new(app.package_query.clone()).block(
        Block::default().borders(Borders::ALL).title(app.tr(
            " tìm gói (/) — Esc xoá ",
            " search (/) — Esc clears ",
            " 搜索（/）— Esc 清除 ",
        )),
    );
    f.render_widget(search, chunks[1]);

    let input = Paragraph::new(app.custom_package_input.clone()).block(
        Block::default().borders(Borders::ALL).title(app.tr(
            " add a package by name (Tab) ",
            " add a package by name (Tab) ",
            " 按名称添加软件包（Tab） ",
        )),
    );
    f.render_widget(input, chunks[2]);

    let builder = Paragraph::new(app.aur_build_user.clone()).block(
        Block::default().borders(Borders::ALL).title(app.tr(
            " tài khoản dựng AUR — KHÔNG BAO GIỜ root ",
            " AUR build account — never root ",
            " AUR 构建账户 — 绝不使用 root ",
        )),
    );
    f.render_widget(builder, chunks[3]);
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
        if app.preview {
            // Asking for the phrase in a preview would be theatre: there is
            // nothing to protect. Saying so is more honest than showing a box
            // that no longer decides anything.
            lines.push(Line::from(Span::styled(
                app.tr(
                    " Xem trước: không cần gõ cụm xác nhận, vì không gì được ghi.",
                    " Preview: no confirmation phrase needed — nothing is written.",
                    " 预览：无需输入确认短语，因为不会写入任何内容。",
                ),
                Style::default().fg(BAD).add_modifier(Modifier::BOLD),
            )));
        } else {
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
        }
    } else {
        lines.push(Line::from(" No plan yet — open the layout step first."));
    }
    let p = Paragraph::new(lines)
        .block(screen_block(app.tr(
            "Xem lại kế hoạch (Enter để cài)",
            "Review plan (Enter to install)",
            "复核方案（Enter 开始安装）",
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
    // The warning is repeated as the first line of the log, not just in the
    // banner: this is the one screen that fills up with command lines, and it is
    // the screen a user glances at while the "install" appears to work.
    let mut lines: Vec<ListItem> = Vec::new();
    if app.preview {
        lines.push(ListItem::new(Line::from(Span::styled(
            format!("⚠ {PREVIEW_BANNER}"),
            Style::default().fg(BAD).add_modifier(Modifier::BOLD),
        ))));
    }
    lines.extend(
        app.install_output
            .iter()
            .map(|l| ListItem::new(Line::from(l.clone()))),
    );
    let list = List::new(lines).block(screen_block(if app.preview {
        app.tr("Tiến trình xem trước", "Preview progress", "预览进度")
    } else {
        app.tr("Tiến trình", "Progress", "进度")
    }));
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
    // The recovery section is the point of the screen when the run stopped:
    // what finished, what did not, what may be retried, what to look at by
    // hand — and, said plainly, that this program cannot promise it will boot.
    if let Some(advice) = &app.recovery {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            " Recovery",
            Style::default().fg(WARN).add_modifier(Modifier::BOLD),
        )));
        for line in advice.render().lines() {
            lines.push(Line::from(format!("  {}", line.trim_end())));
        }
    }
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        app.tr_owned(
            " d — lưu báo cáo chẩn đoán (không có mật khẩu) · Enter — thoát ",
            " d — save a diagnostics report (no secrets) · Enter — quit ",
            " d — 保存诊断报告（不含机密）· Enter — 退出 ",
        ),
        Style::default().fg(ACCENT),
    )));
    let p = Paragraph::new(lines)
        .block(screen_block("Result"))
        .wrap(Wrap { trim: false });
    f.render_widget(p, area);
}

fn draw_help(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = vec![
        Line::from(""),
        Line::from("  ↑ / ↓      move / navigate"),
        Line::from("  Enter      select, or start the step"),
        Line::from("  Space      toggle an option"),
        Line::from("  Tab        next field"),
        Line::from("  /          search the package catalog"),
        Line::from("  L          load a saved profile (welcome screen)"),
        Line::from("  d          write a diagnostics report (result screen)"),
        Line::from("  Esc        back (before anything irreversible)"),
        Line::from("  F1         this help"),
        Line::from("  F5         re-scan hardware and disks"),
        Line::from("  F10        review the plan"),
        Line::from("  Ctrl-C     quit"),
        Line::from(""),
        Line::from("  camareLOL never writes to a disk until you type the confirmation"),
        Line::from("  phrase on the review screen, and it re-checks the disk identity"),
        Line::from("  immediately before every write."),
        Line::from(""),
        Line::from("  A saved profile never contains a password. Loading one restores"),
        Line::from("  every other choice; passwords are typed again on the accounts step."),
    ];
    if app.preview {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("  ⚠ {PREVIEW_BANNER}"),
            Style::default().fg(BAD).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(
            "  --preview runs every phase and prints every command, and runs none of them.",
        ));
    }
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

#[cfg(test)]
mod tests {
    use crate::app::{App, Screen};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn welcome_app() -> App {
        let dir = std::env::temp_dir().join("camarelol-draw-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("draw-{}.json", std::process::id()));
        std::fs::write(&path, "[]").unwrap();
        let mut app = App::new(Some(path));
        app.screen = Screen::Welcome;
        app
    }

    /// Render the welcome screen and return it as plain text.
    fn welcome_text() -> String {
        render(welcome_app(), Screen::Welcome, 90, 24)
    }

    /// Draw `app` on a screen of the given size and return the visible text.
    ///
    /// A double-width grapheme occupies two cells and the second is a plain
    /// space, indistinguishable from padding by content alone. Walking the
    /// buffer by display width drops it, so a Chinese string comes back as
    /// `简体中文` and a test can assert on it. Naively concatenating every cell
    /// produced `简 体 中 文`, which no assertion could match.
    fn render(mut app: App, screen: Screen, width: u16, height: u16) -> String {
        app.screen = screen;
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|f| super::draw(f, &mut app)).unwrap();
        let buf = terminal.backend().buffer().clone();
        use unicode_width::UnicodeWidthStr;
        let w = width as usize;
        // The cell grid is fixed: exactly `width` cells per row. The walk has
        // to chunk first and drop the trailing cells of a wide grapheme
        // *inside* the chunk, otherwise the row boundary moves and every row
        // below it is shifted.
        buf.content()
            .chunks(w)
            .map(|cells| {
                let mut row = String::new();
                let mut i = 0;
                while i < cells.len() {
                    let symbol = cells[i].symbol();
                    let cell_width = symbol.width().max(1);
                    row.push_str(symbol);
                    i += cell_width;
                }
                row
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_welcome_screen_says_how_to_update() {
        // It used to say "press u", then it said `sudo pacman -S camarelol` —
        // and there is no such package, so that instruction could never work.
        // The honest answer is the one-liner that really reinstalls it.
        let text = welcome_text();
        assert!(text.contains("camarelol.sh"), "got:\n{text}");
        assert!(!text.contains("pacman"), "không có gói pacman:\n{text}");
    }

    #[test]
    fn the_welcome_screen_does_not_mention_an_update_flag() {
        let text = welcome_text();
        assert!(!text.contains("--update"), "got:\n{text}");
        assert!(!text.contains("Press u"), "got:\n{text}");
    }

    #[test]
    fn no_banner_appears_without_preview() {
        // The banner must mean something. A normal run has to look normal.
        let mut app = welcome_app();
        app.preview = false;
        let text = render(app, Screen::Welcome, 100, 30);
        assert!(!text.contains(super::PREVIEW_BANNER), "got:\n{text}");
        assert!(!text.contains("PREVIEW"), "got:\n{text}");
    }

    #[test]
    fn the_preview_banner_is_on_every_screen() {
        // Every screen, including Progress: a warning that flickers off while
        // the install appears to run is worse than no warning, because it
        // teaches the user that the warning is optional.
        let screens = [
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
            Screen::Help,
            Screen::Logs,
        ];
        for screen in screens {
            let mut app = welcome_app();
            app.preview = true;
            app.install_output
                .push("DRYRUN sfdisk --force /dev/vdb".into());
            let text = render(app, screen, 120, 40);
            assert!(
                text.contains(super::PREVIEW_BANNER),
                "no preview banner on {screen:?}:\n{text}"
            );
            // And the safety gate in the footer must not claim EXECUTE.
            assert!(
                !text.contains("EXECUTE"),
                "the footer claims EXECUTE on {screen:?}"
            );
        }
    }

    #[test]
    fn the_preview_banner_survives_a_narrow_terminal() {
        // 40 columns is narrow but legal (the minimum is 30). The warning is
        // the one string that must still be there.
        let mut app = welcome_app();
        app.preview = true;
        let text = render(app, Screen::Progress, 40, 24);
        assert!(
            text.contains("PREVIEW"),
            "the warning vanished on a narrow terminal:\n{text}"
        );
    }

    #[test]
    fn the_review_screen_does_not_ask_for_a_confirmation_in_preview() {
        // Asking for `ERASE` in a run that cannot erase anything is theatre, and
        // it is worse than theatre: it suggests the phrase is what stops the
        // writing.
        let mut app = welcome_app();
        app.preview = true;
        let text = render(app, Screen::Review, 120, 40);
        assert!(!text.contains("Type ERASE"), "got:\n{text}");
    }

    // ------------------------------------------------------------ the host

    #[test]
    fn the_welcome_screen_names_the_host_it_detected() {
        // Distribution and init on the first screen, not at the end of a wizard:
        // the user has to be told what machine they are on before they answer
        // anything else. Both states are asserted, because the screen renders
        // them differently — a `Host:` row when it is supported, a warning that
        // names it when it is not.
        let mut app = welcome_app();
        app.host = camarelol_core::host::HostProfile::arch_systemd();
        let text = render(app, Screen::Welcome, 120, 40);
        assert!(text.contains("Host:"), "got:\n{text}");
        assert!(text.contains("Arch Linux / systemd"), "got:\n{text}");

        let mut app = welcome_app();
        app.host = refused_host();
        let text = render(app, Screen::Welcome, 120, 40);
        assert!(
            text.contains("Hyperbola GNU/Linux-libre / systemd"),
            "an unsupported host must still be named on screen:\n{text}"
        );
    }

    #[test]
    fn an_unsupported_host_is_announced_before_the_user_answers_anything() {
        let mut app = welcome_app();
        app.host = refused_host();
        let text = render(app, Screen::Welcome, 120, 40);
        assert!(text.contains("Unsupported host"), "got:\n{text}");
        assert!(text.contains("Hyperbola"), "got:\n{text}");
        assert!(text.contains("Arch live ISO"), "got:\n{text}");
    }

    #[test]
    fn a_supported_host_gets_no_warning_on_the_welcome_screen() {
        let mut app = welcome_app();
        app.host = camarelol_core::host::HostProfile::arch_systemd();
        let text = render(app, Screen::Welcome, 120, 40);
        assert!(!text.contains("Unsupported host"), "got:\n{text}");
        assert!(text.contains("Arch Linux / systemd"), "got:\n{text}");
    }

    #[test]
    fn the_welcome_screen_still_fits_a_twenty_four_row_terminal() {
        // The update line used to be the last thing on the screen; a warning
        // block that wrapped onto three rows pushed it off. Both states are
        // checked because the refused one is the longer of the two.
        for host in [
            camarelol_core::host::HostProfile::arch_systemd(),
            refused_host(),
        ] {
            let mut app = welcome_app();
            app.host = host;
            app.wifi_needs_setup = true;
            let text = render(app, Screen::Welcome, 90, 24);
            assert!(
                text.contains("camarelol.sh"),
                "the update line fell off the screen:\n{text}"
            );
        }
    }

    /// A profile the decision table refuses, so the drawing tests do not depend
    /// on the machine they run on.
    fn refused_host() -> camarelol_core::host::HostProfile {
        camarelol_core::host::HostProfile::from_parts(
            &camarelol_core::host::parse_os_release(
                "NAME=\"Hyperbola GNU/Linux-libre\"\nID=hyperbola\nID_LIKE=\"parabola arch\"\n",
            ),
            camarelol_core::host::InitSystem::Systemd,
        )
    }

    // -------------------------------------------------------------- zh

    #[test]
    fn chinese_is_offered_in_the_language_picker() {
        let mut app = welcome_app();
        app.screen = Screen::Language;
        app.language_index = 2;
        let text = render(app, Screen::Language, 100, 30);
        assert!(text.contains("Tiếng Việt"), "got:\n{text}");
        assert!(text.contains("English"), "got:\n{text}");
        assert!(text.contains("简体中文"), "got:\n{text}");
    }

    #[test]
    fn the_setup_screens_render_in_chinese() {
        // Not a translation-quality test: a smoke test that the third language
        // reaches every screen and that `tr` never returns an empty string for
        // it. An empty branch would render as a blank title.
        for screen in [
            Screen::Language,
            Screen::Keyboard,
            Screen::Timezone,
            Screen::Layout,
            Screen::Accounts,
            Screen::Desktop,
            Screen::Services,
            Screen::Packages,
            Screen::Review,
            Screen::Filesystem,
            Screen::Bootloader,
        ] {
            let mut app = welcome_app();
            app.language_index = 2;
            app.screen = screen;
            let text = render(app, screen, 120, 40);
            assert!(
                !text.trim().is_empty(),
                "the {screen:?} screen is blank in Chinese"
            );
            assert!(
                text.contains("camareLOL") || text.contains(">"),
                "the {screen:?} screen lost its frame in Chinese:\n{text}"
            );
        }
    }

    #[test]
    fn the_chinese_review_screen_uses_the_documentation_vocabulary() {
        // Terminology is taken from `docs/zh/index.md`, so a user who read the
        // documentation recognises the screens.
        let mut app = welcome_app();
        app.language_index = 2;
        app.screen = Screen::Review;
        app.selected_disk = Some("/dev/vdb".into());
        app.plan = None;
        let text = render(app, Screen::Review, 120, 40);
        assert!(text.contains("复核方案"), "got:\n{text}");
    }

    #[test]
    fn the_chinese_accounts_screen_uses_the_documentation_vocabulary() {
        let mut app = welcome_app();
        app.language_index = 2;
        app.screen = Screen::Accounts;
        let text = render(app, Screen::Accounts, 120, 40);
        assert!(text.contains("用户账户"), "got:\n{text}");
        assert!(text.contains("密码只保存在内存中"), "got:\n{text}");
    }

    #[test]
    fn the_chinese_footer_names_the_safety_gate() {
        // The gate is the one string that must be translated: a user who cannot
        // read it cannot tell EXECUTE from dry-run.
        let mut app = welcome_app();
        app.language_index = 2;
        app.preview = true;
        let text = render(app, Screen::Welcome, 140, 40);
        assert!(text.contains("安全闸门"), "got:\n{text}");
        assert!(
            text.contains("PREVIEW"),
            "the banner must survive zh: got:\n{text}"
        );
    }
}
