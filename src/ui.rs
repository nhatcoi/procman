use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table},
    Frame, Terminal,
};
use std::collections::HashMap;
use std::io::stdout;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::cloudflare::{forward_start, forward_stop};
use crate::config::{find_config_path, Config};
use crate::logs::read_tail;
use crate::paths::global_data_root;
use crate::process_manager::{
    self, force_kill_by_pid, log_file_for, stop_by_pid, GlobalProcRow, StatusRow,
};
use crate::qr::render_qr;

const NORMAL_LOG_LINES: usize = 18;
const FULLSCREEN_LOG_LINES: usize = 500;
const UI_REFRESH_INTERVAL: Duration = Duration::from_millis(500);
const EVENT_POLL_TIMEOUT: Duration = Duration::from_millis(50);
const SCROLL_STEP_PAGE: usize = 15;
const DOT_RUNNING: &str = "●";
const DOT_STOPPED: &str = "○";
const DOT_PENDING: &str = "⟳";
const DEFAULT_PLACEHOLDER: &str = "-";

struct AppState {
    is_global_mode: bool,
    selected_idx: usize,
    status_msg: String,
    show_qr_modal: bool,
    is_fullscreen_log: bool,
    is_search_input: bool,
    search_query: String,
    scroll_offset: usize,
    local_rows: Vec<StatusRow>,
    global_rows: Vec<GlobalProcRow>,
    pending_actions: Arc<Mutex<HashMap<String, String>>>,
    metrics: crate::metrics::ProcessMetrics,
    last_refresh: Instant,
}

impl AppState {
    fn new(start_all: bool, has_local_config: bool) -> Self {
        Self {
            is_global_mode: start_all || !has_local_config,
            selected_idx: 0,
            status_msg: "ready".to_string(),
            show_qr_modal: false,
            is_fullscreen_log: false,
            is_search_input: false,
            search_query: String::new(),
            scroll_offset: 0,
            local_rows: Vec::new(),
            global_rows: Vec::new(),
            pending_actions: Arc::new(Mutex::new(HashMap::new())),
            metrics: crate::metrics::ProcessMetrics::new(),
            last_refresh: Instant::now() - UI_REFRESH_INTERVAL,
        }
    }

    fn refresh_rows(&mut self, local_cfg: Option<(&Path, &Config)>) {
        self.metrics.refresh();
        if let Some((cp, cfg)) = local_cfg {
            self.local_rows =
                process_manager::status_with_metrics(cp, cfg, None, &mut self.metrics)
                    .unwrap_or_default();
        }
        self.global_rows = process_manager::scan_global_processes_with_metrics(&mut self.metrics)
            .unwrap_or_default();

        // Auto-clear pending actions once the actual status is observed
        if let Ok(mut pending) = self.pending_actions.lock() {
            pending.retain(|key, action| {
                if action == "starting..." {
                    // if now running, clear pending
                    let is_up = if let Some(slash_idx) = key.find('/') {
                        let (p_key, s_name) = key.split_at(slash_idx);
                        let s_name = &s_name[1..];
                        self.global_rows.iter().any(|r| {
                            r.project_key == p_key && r.service_name == s_name && r.running
                        })
                    } else {
                        self.local_rows.iter().any(|r| r.name == *key && r.running)
                    };
                    !is_up
                } else if action == "stopping..." {
                    let is_down = if let Some(slash_idx) = key.find('/') {
                        let (p_key, s_name) = key.split_at(slash_idx);
                        let s_name = &s_name[1..];
                        self.global_rows.iter().any(|r| {
                            r.project_key == p_key && r.service_name == s_name && !r.running
                        })
                    } else {
                        self.local_rows.iter().any(|r| r.name == *key && !r.running)
                    };
                    !is_down
                } else {
                    false
                }
            });
        }

        self.last_refresh = Instant::now();
    }

    fn clamp_selected_idx(&mut self, count: usize) {
        if count == 0 {
            self.selected_idx = 0;
        } else if self.selected_idx >= count {
            self.selected_idx = count - 1;
        }
    }
}

pub fn render_ui(start_all: bool) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, start_all);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn highlight_line<'a>(line: &'a str, query: &str) -> Line<'a> {
    if query.is_empty() {
        return Line::from(Span::raw(line));
    }

    let mut spans = Vec::new();
    let lower_line = line.to_lowercase();
    let lower_query = query.to_lowercase();

    let mut last_idx = 0;
    for (match_start, _) in lower_line.match_indices(&lower_query) {
        if match_start > last_idx {
            spans.push(Span::raw(&line[last_idx..match_start]));
        }
        let match_end = match_start + query.len();
        spans.push(Span::styled(
            &line[match_start..match_end],
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
        last_idx = match_end;
    }

    if last_idx < line.len() {
        spans.push(Span::raw(&line[last_idx..]));
    }

    if spans.is_empty() {
        Line::from(Span::raw(line))
    } else {
        Line::from(spans)
    }
}

fn render_global_table(f: &mut Frame, area: Rect, state: &AppState) {
    let pending_guard = state.pending_actions.lock().ok();

    let table_rows: Vec<Row> = state
        .global_rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let is_selected = i == state.selected_idx;
            let key = format!("{}/{}", r.project_key, r.service_name);
            let pending_status = pending_guard.as_ref().and_then(|p| p.get(&key));

            let (dot, dot_color, pid_str) = if let Some(status) = pending_status {
                (DOT_PENDING, Color::Yellow, status.clone())
            } else if r.running {
                (
                    DOT_RUNNING,
                    Color::Green,
                    r.pid
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| DEFAULT_PLACEHOLDER.into()),
                )
            } else {
                (DOT_STOPPED, Color::Red, DEFAULT_PLACEHOLDER.into())
            };

            let cpu_str = if pending_status.is_some() {
                DEFAULT_PLACEHOLDER.into()
            } else {
                r.cpu
                    .map(|c| format!("{:.1}%", c))
                    .unwrap_or_else(|| DEFAULT_PLACEHOLDER.into())
            };

            let mem_str = if pending_status.is_some() {
                DEFAULT_PLACEHOLDER.into()
            } else {
                r.memory_mb
                    .map(|m| format!("{}MB", m))
                    .unwrap_or_else(|| DEFAULT_PLACEHOLDER.into())
            };

            let up_str = if pending_status.is_some() {
                DEFAULT_PLACEHOLDER.into()
            } else {
                r.uptime
                    .clone()
                    .unwrap_or_else(|| DEFAULT_PLACEHOLDER.into())
            };

            let port_str = r
                .port
                .map(|p| p.to_string())
                .unwrap_or_else(|| DEFAULT_PLACEHOLDER.into());
            let tunnel_str = r.tunnel_url.clone().unwrap_or_default();

            let row = Row::new(vec![
                Line::from(vec![
                    Span::styled(format!("{} ", dot), Style::default().fg(dot_color)),
                    Span::raw(r.project_key.clone()),
                ]),
                Line::from(r.service_name.clone()),
                Line::from(if pending_status.is_some() {
                    Span::styled(
                        pid_str,
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::ITALIC),
                    )
                } else {
                    Span::raw(pid_str)
                }),
                Line::from(cpu_str),
                Line::from(mem_str),
                Line::from(up_str),
                Line::from(port_str),
                Line::from(tunnel_str),
            ]);

            if is_selected {
                row.style(
                    Style::default()
                        .add_modifier(Modifier::REVERSED)
                        .fg(Color::Yellow),
                )
            } else {
                row
            }
        })
        .collect();

    let header = Row::new(vec![
        "PROJECT",
        "SERVICE",
        "STATUS/PID",
        "CPU",
        "MEM",
        "UPTIME",
        "PORT",
        "TUNNEL",
    ])
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let table = Table::new(
        table_rows,
        [
            Constraint::Percentage(20),
            Constraint::Percentage(14),
            Constraint::Percentage(11),
            Constraint::Percentage(7),
            Constraint::Percentage(7),
            Constraint::Percentage(9),
            Constraint::Percentage(8),
            Constraint::Percentage(24),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(format!(
                " 🌐 Global Dashboard (All Projects - {} services) [Tab: Switch View] ",
                state.global_rows.len()
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::LightMagenta)),
    );

    f.render_widget(table, area);
}

fn render_local_table(f: &mut Frame, area: Rect, config_path: &Path, state: &AppState) {
    let pending_guard = state.pending_actions.lock().ok();

    let table_rows: Vec<Row> = state
        .local_rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let is_selected = i == state.selected_idx;
            let pending_status = pending_guard.as_ref().and_then(|p| p.get(&r.name));

            let (dot, dot_color, pid_str) = if let Some(status) = pending_status {
                (DOT_PENDING, Color::Yellow, status.clone())
            } else if r.running {
                (
                    DOT_RUNNING,
                    Color::Green,
                    r.pid
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| DEFAULT_PLACEHOLDER.into()),
                )
            } else {
                (DOT_STOPPED, Color::Red, DEFAULT_PLACEHOLDER.into())
            };

            let cpu_str = if pending_status.is_some() {
                DEFAULT_PLACEHOLDER.into()
            } else {
                r.cpu
                    .map(|c| format!("{:.1}%", c))
                    .unwrap_or_else(|| DEFAULT_PLACEHOLDER.into())
            };

            let mem_str = if pending_status.is_some() {
                DEFAULT_PLACEHOLDER.into()
            } else {
                r.memory_mb
                    .map(|m| format!("{}MB", m))
                    .unwrap_or_else(|| DEFAULT_PLACEHOLDER.into())
            };

            let up_str = if pending_status.is_some() {
                DEFAULT_PLACEHOLDER.into()
            } else {
                r.uptime
                    .clone()
                    .unwrap_or_else(|| DEFAULT_PLACEHOLDER.into())
            };

            let port_str = r
                .port
                .map(|p| p.to_string())
                .unwrap_or_else(|| DEFAULT_PLACEHOLDER.into());
            let tunnel_str = r.tunnel_url.clone().unwrap_or_default();

            let row = Row::new(vec![
                Line::from(vec![
                    Span::styled(format!("{} ", dot), Style::default().fg(dot_color)),
                    Span::raw(r.name.clone()),
                ]),
                Line::from(if pending_status.is_some() {
                    Span::styled(
                        pid_str,
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::ITALIC),
                    )
                } else {
                    Span::raw(pid_str)
                }),
                Line::from(cpu_str),
                Line::from(mem_str),
                Line::from(up_str),
                Line::from(port_str),
                Line::from(tunnel_str),
            ]);

            if is_selected {
                row.style(
                    Style::default()
                        .add_modifier(Modifier::REVERSED)
                        .fg(Color::Yellow),
                )
            } else {
                row
            }
        })
        .collect();

    let header = Row::new(vec![
        "NAME",
        "STATUS/PID",
        "CPU",
        "MEM",
        "UPTIME",
        "PORT",
        "TUNNEL",
    ])
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let table = Table::new(
        table_rows,
        [
            Constraint::Percentage(20),
            Constraint::Percentage(12),
            Constraint::Percentage(8),
            Constraint::Percentage(8),
            Constraint::Percentage(12),
            Constraint::Percentage(8),
            Constraint::Percentage(32),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .title(format!(
                " 📁 Project ({}) [Tab: Switch View] ",
                config_path.display()
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray)),
    );

    f.render_widget(table, area);
}

fn build_help_spans(state: &AppState) -> Vec<Span<'static>> {
    if state.is_search_input {
        vec![
            Span::styled(
                "Search: ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{}_", state.search_query),
                Style::default().fg(Color::White),
            ),
            Span::raw("  (Enter to filter, Esc to cancel)"),
        ]
    } else if state.is_fullscreen_log {
        vec![
            Span::styled(
                "Enter/Space",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" exit fullscreen  "),
            Span::styled(
                "↑/↓/PgUp/PgDn",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" scroll  "),
            Span::styled(
                "G",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" latest  "),
            Span::styled(
                "/",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" search  "),
            Span::styled(
                "c",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" clear filter  "),
            Span::styled(
                "q",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" quit"),
        ]
    } else if state.is_global_mode {
        vec![
            Span::styled(
                "Tab",
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" local view  "),
            Span::styled(
                "↑/↓",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" select  "),
            Span::styled(
                "Enter",
                Style::default()
                    .fg(Color::LightBlue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" zoom log  "),
            Span::styled(
                "/",
                Style::default()
                    .fg(Color::LightYellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" search  "),
            Span::styled(
                "s",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" start  "),
            Span::styled(
                "x",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" stop  "),
            Span::styled(
                "k",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" kill  "),
            Span::styled(
                "r",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" restart  "),
            Span::styled(
                "f",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" forward  "),
            Span::styled(
                "o",
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" QR  "),
            Span::styled(
                "d",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" unregister  "),
            Span::styled(
                "q",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" quit"),
        ]
    } else {
        vec![
            Span::styled(
                "Tab",
                Style::default()
                    .fg(Color::LightMagenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" all projects  "),
            Span::styled(
                "↑/↓",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" select  "),
            Span::styled(
                "Enter",
                Style::default()
                    .fg(Color::LightBlue)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" zoom log  "),
            Span::styled(
                "/",
                Style::default()
                    .fg(Color::LightYellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" search  "),
            Span::styled(
                "s",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" start  "),
            Span::styled(
                "x",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" stop  "),
            Span::styled(
                "k",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" kill  "),
            Span::styled(
                "r",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" restart  "),
            Span::styled(
                "f",
                Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" forward  "),
            Span::styled(
                "o",
                Style::default()
                    .fg(Color::LightCyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" QR  "),
            Span::styled(
                "q",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" quit"),
        ]
    }
}

fn render_qr_popup(f: &mut Frame, area: Rect, title: &str, url: &str) {
    let qr_code_str = render_qr(url).unwrap_or_else(|| "Failed to generate QR".to_string());
    let popup_area = centered_rect(70, 80, area);

    f.render_widget(Clear, popup_area);

    let mut qr_lines: Vec<Line> = qr_code_str
        .lines()
        .map(|l| Line::from(Span::raw(l.to_string())))
        .collect();

    qr_lines.insert(
        0,
        Line::from(vec![
            Span::styled(
                "Scan with Mobile Camera: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(url, Style::default().fg(Color::Yellow)),
        ]),
    );
    qr_lines.insert(1, Line::from(""));
    qr_lines.push(Line::from(""));
    qr_lines.push(Line::from(Span::styled(
        "Press 'o' or 'Esc' to close",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC),
    )));

    let popup_block = Block::default()
        .title(format!(" QR Code: {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));

    let popup_paragraph = Paragraph::new(qr_lines)
        .block(popup_block)
        .alignment(Alignment::Center);

    f.render_widget(popup_paragraph, popup_area);
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    start_all: bool,
) -> Result<()> {
    let local_config_opt = find_config_path(None)
        .and_then(|p| crate::config::load_config(&p).ok().map(|cfg| (p, cfg)));

    let mut state = AppState::new(start_all, local_config_opt.is_some());

    loop {
        if state.last_refresh.elapsed() >= UI_REFRESH_INTERVAL {
            let local_ref = local_config_opt.as_ref().map(|(p, c)| (p.as_path(), c));
            state.refresh_rows(local_ref);
        }

        let (display_title, log_file, selected_url, row_count) = if state.is_global_mode {
            let count = state.global_rows.len();
            state.clamp_selected_idx(count);

            let selected_global = state.global_rows.get(state.selected_idx);
            let (title_name, target_log, url) = if let Some(g) = selected_global {
                let log_path = global_data_root()
                    .join(&g.project_key)
                    .join("logs")
                    .join(format!("{}.log", g.service_name));
                let target_url = g
                    .tunnel_url
                    .clone()
                    .or_else(|| g.port.map(|p| format!("http://localhost:{}", p)));
                (
                    format!("{}/{}", g.project_key, g.service_name),
                    log_path,
                    target_url,
                )
            } else {
                ("none".to_string(), PathBuf::new(), None)
            };

            (title_name, target_log, url, count)
        } else {
            let (ref cp, _) = local_config_opt.as_ref().unwrap();
            let count = state.local_rows.len();
            state.clamp_selected_idx(count);

            let selected_local = state.local_rows.get(state.selected_idx);
            let (title_name, target_log, url) = if let Some(r) = selected_local {
                let log_p = log_file_for(cp, &r.name).unwrap_or_default();
                let target_url = r
                    .tunnel_url
                    .clone()
                    .or_else(|| r.port.map(|p| format!("http://localhost:{}", p)));
                (r.name.clone(), log_p, target_url)
            } else {
                ("none".to_string(), PathBuf::new(), None)
            };

            (title_name, target_log, url, count)
        };

        let lines_to_read = if state.is_fullscreen_log {
            FULLSCREEN_LOG_LINES
        } else {
            NORMAL_LOG_LINES
        };
        let raw_tail = if log_file.is_file() {
            read_tail(&log_file, lines_to_read)
        } else {
            "(no logs recorded yet)".to_string()
        };

        let filtered_lines: Vec<&str> = if state.search_query.is_empty() {
            raw_tail.lines().collect()
        } else {
            let lower_query = state.search_query.to_lowercase();
            raw_tail
                .lines()
                .filter(|l| l.to_lowercase().contains(&lower_query))
                .collect()
        };

        let total_log_count = filtered_lines.len();

        let visible_slice = if state.scroll_offset > 0 && total_log_count > state.scroll_offset {
            let end = total_log_count - state.scroll_offset;
            &filtered_lines[..end]
        } else {
            &filtered_lines[..]
        };

        let styled_log_lines: Vec<Line> = visible_slice
            .iter()
            .map(|l| highlight_line(l, &state.search_query))
            .collect();

        terminal.draw(|f| {
            let area = f.area();

            if state.is_fullscreen_log {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Min(8),
                        Constraint::Length(1),
                        Constraint::Length(1),
                    ])
                    .split(area);

                let search_badge = if !state.search_query.is_empty() {
                    format!(
                        " [Filter: \"{}\" ({} matches)]",
                        state.search_query, total_log_count
                    )
                } else {
                    String::new()
                };
                let scroll_badge = if state.scroll_offset > 0 {
                    format!(" [Scrolled +{}]", state.scroll_offset)
                } else {
                    String::new()
                };

                let mode_badge = if state.is_global_mode {
                    "[GLOBAL VIEW] "
                } else {
                    "[LOCAL VIEW] "
                };
                let log_title = format!(
                    " FULLSCREEN LOG: {}{} {}{}- (Enter/Esc to exit) ",
                    mode_badge, display_title, search_badge, scroll_badge
                );
                let log_paragraph = Paragraph::new(styled_log_lines).block(
                    Block::default()
                        .title(log_title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)),
                );
                f.render_widget(log_paragraph, chunks[0]);

                let help_spans = build_help_spans(&state);
                f.render_widget(Paragraph::new(Line::from(help_spans)), chunks[1]);

                let msg_line = Line::from(vec![
                    Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        state.status_msg.clone(),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]);
                f.render_widget(Paragraph::new(msg_line), chunks[2]);
            } else {
                let table_height = (row_count as u16 + 3).max(5).min(area.height / 2);
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Length(table_height),
                        Constraint::Min(6),
                        Constraint::Length(1),
                        Constraint::Length(1),
                    ])
                    .split(area);

                if state.is_global_mode {
                    render_global_table(f, chunks[0], &state);
                } else {
                    let (ref cp, _) = local_config_opt.as_ref().unwrap();
                    render_local_table(f, chunks[0], cp, &state);
                }

                let search_badge = if !state.search_query.is_empty() {
                    format!(
                        " [Filter: \"{}\" ({} matches)]",
                        state.search_query, total_log_count
                    )
                } else {
                    String::new()
                };

                let log_title = format!(
                    " log: {} {} (Enter: fullscreen, /: search) ",
                    display_title, search_badge
                );
                let log_paragraph = Paragraph::new(styled_log_lines).block(
                    Block::default()
                        .title(log_title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)),
                );
                f.render_widget(log_paragraph, chunks[1]);

                let help_spans = build_help_spans(&state);
                f.render_widget(Paragraph::new(Line::from(help_spans)), chunks[2]);

                let msg_line = Line::from(vec![
                    Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        state.status_msg.clone(),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]);
                f.render_widget(Paragraph::new(msg_line), chunks[3]);
            }

            if state.show_qr_modal {
                if let Some(ref url) = selected_url {
                    render_qr_popup(f, area, &display_title, url);
                }
            }
        })?;

        if event::poll(EVENT_POLL_TIMEOUT)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if state.show_qr_modal {
                        match key.code {
                            KeyCode::Char('o')
                            | KeyCode::Char('O')
                            | KeyCode::Char('q')
                            | KeyCode::Esc => {
                                state.show_qr_modal = false;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    if state.is_search_input {
                        match key.code {
                            KeyCode::Esc => {
                                state.is_search_input = false;
                                state.search_query.clear();
                                state.scroll_offset = 0;
                            }
                            KeyCode::Enter => {
                                state.is_search_input = false;
                                state.scroll_offset = 0;
                                state.status_msg = if state.search_query.is_empty() {
                                    "search filter cleared".to_string()
                                } else {
                                    format!("filter applied: \"{}\"", state.search_query)
                                };
                            }
                            KeyCode::Backspace => {
                                state.search_query.pop();
                            }
                            KeyCode::Char(c) => {
                                state.search_query.push(c);
                            }
                            _ => {}
                        }
                        continue;
                    }

                    if state.is_fullscreen_log {
                        match key.code {
                            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Esc => {
                                state.is_fullscreen_log = false;
                                state.scroll_offset = 0;
                            }
                            KeyCode::Char('/') => {
                                state.is_search_input = true;
                            }
                            KeyCode::Char('c') | KeyCode::Char('C') => {
                                state.search_query.clear();
                                state.scroll_offset = 0;
                                state.status_msg = "filter cleared".to_string();
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                state.scroll_offset = state.scroll_offset.saturating_add(1);
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                state.scroll_offset = state.scroll_offset.saturating_sub(1);
                            }
                            KeyCode::PageUp => {
                                state.scroll_offset =
                                    state.scroll_offset.saturating_add(SCROLL_STEP_PAGE);
                            }
                            KeyCode::PageDown => {
                                state.scroll_offset =
                                    state.scroll_offset.saturating_sub(SCROLL_STEP_PAGE);
                            }
                            KeyCode::Home | KeyCode::Char('g') => {
                                state.scroll_offset = total_log_count.saturating_sub(10);
                            }
                            KeyCode::End | KeyCode::Char('G') => {
                                state.scroll_offset = 0;
                            }
                            KeyCode::Char('q') => break,
                            _ => {}
                        }
                        continue;
                    }

                    match key.code {
                        KeyCode::Tab => {
                            if local_config_opt.is_some() {
                                state.is_global_mode = !state.is_global_mode;
                                state.selected_idx = 0;
                                state.scroll_offset = 0;
                                state.search_query.clear();
                                state.status_msg = if state.is_global_mode {
                                    "switched to Global Dashboard (all projects)".to_string()
                                } else {
                                    "switched to Local Project view".to_string()
                                };
                            } else {
                                state.status_msg =
                                    "No local procman.yaml (staying in Global View)".to_string();
                            }
                        }
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Enter | KeyCode::Char(' ') => {
                            if row_count > 0 {
                                state.is_fullscreen_log = true;
                                state.scroll_offset = 0;
                            }
                        }
                        KeyCode::Char('/') => {
                            state.is_search_input = true;
                        }
                        KeyCode::Char('c') | KeyCode::Char('C') => {
                            state.search_query.clear();
                            state.scroll_offset = 0;
                            state.status_msg = "filter cleared".to_string();
                        }
                        KeyCode::Char('o') | KeyCode::Char('O') => {
                            if selected_url.is_some() {
                                state.show_qr_modal = !state.show_qr_modal;
                            } else {
                                state.status_msg =
                                    format!("no tunnel URL or port for {}", display_title);
                            }
                        }
                        KeyCode::Up => {
                            if state.selected_idx > 0 {
                                state.selected_idx -= 1;
                            } else if row_count > 0 {
                                state.selected_idx = row_count - 1;
                            }
                        }
                        KeyCode::Down => {
                            if row_count > 0 {
                                state.selected_idx = (state.selected_idx + 1) % row_count;
                            }
                        }
                        KeyCode::Char('s') => {
                            if state.is_global_mode {
                                if let Some(g) = state.global_rows.get(state.selected_idx).cloned()
                                {
                                    if let Some(ref cp) = g.config_path {
                                        let key = format!("{}/{}", g.project_key, g.service_name);
                                        if let Ok(mut pending) = state.pending_actions.lock() {
                                            pending.insert(key, "starting...".to_string());
                                        }
                                        state.status_msg = format!(
                                            "⟳ starting {} in {}...",
                                            g.service_name, g.project_key
                                        );

                                        let cp_clone = cp.clone();
                                        let service_clone = g.service_name.clone();
                                        let pending_clone = Arc::clone(&state.pending_actions);
                                        let key_clone =
                                            format!("{}/{}", g.project_key, g.service_name);

                                        thread::spawn(move || {
                                            if let Ok(cfg) = crate::config::load_config(&cp_clone) {
                                                let _ = process_manager::start(
                                                    &cp_clone,
                                                    &cfg,
                                                    Some(&service_clone),
                                                    false,
                                                );
                                            }
                                            if let Ok(mut p) = pending_clone.lock() {
                                                p.remove(&key_clone);
                                            }
                                        });
                                    }
                                }
                            } else if let Some((ref cp, ref cfg)) = local_config_opt {
                                if let Some(target_name) = state
                                    .local_rows
                                    .get(state.selected_idx)
                                    .map(|r| r.name.clone())
                                {
                                    if let Ok(mut pending) = state.pending_actions.lock() {
                                        pending
                                            .insert(target_name.clone(), "starting...".to_string());
                                    }
                                    state.status_msg = format!("⟳ starting {}...", target_name);

                                    let cp_clone = cp.clone();
                                    let cfg_clone = cfg.clone();
                                    let target_clone = target_name.clone();
                                    let pending_clone = Arc::clone(&state.pending_actions);

                                    thread::spawn(move || {
                                        let _ = process_manager::start(
                                            &cp_clone,
                                            &cfg_clone,
                                            Some(&target_clone),
                                            false,
                                        );
                                        if let Ok(mut p) = pending_clone.lock() {
                                            p.remove(&target_clone);
                                        }
                                    });
                                }
                            }
                        }
                        KeyCode::Char('r') => {
                            if state.is_global_mode {
                                if let Some(g) = state.global_rows.get(state.selected_idx).cloned()
                                {
                                    if let Some(ref cp) = g.config_path {
                                        let key = format!("{}/{}", g.project_key, g.service_name);
                                        if let Ok(mut pending) = state.pending_actions.lock() {
                                            pending.insert(key, "restarting...".to_string());
                                        }
                                        state.status_msg = format!(
                                            "⟳ restarting {} in {}...",
                                            g.service_name, g.project_key
                                        );

                                        let cp_clone = cp.clone();
                                        let service_clone = g.service_name.clone();
                                        let pending_clone = Arc::clone(&state.pending_actions);
                                        let key_clone =
                                            format!("{}/{}", g.project_key, g.service_name);

                                        thread::spawn(move || {
                                            if let Ok(cfg) = crate::config::load_config(&cp_clone) {
                                                let _ = process_manager::restart(
                                                    &cp_clone,
                                                    &cfg,
                                                    Some(&service_clone),
                                                    false,
                                                );
                                            }
                                            if let Ok(mut p) = pending_clone.lock() {
                                                p.remove(&key_clone);
                                            }
                                        });
                                    }
                                }
                            } else if let Some((ref cp, ref cfg)) = local_config_opt {
                                if let Some(target_name) = state
                                    .local_rows
                                    .get(state.selected_idx)
                                    .map(|r| r.name.clone())
                                {
                                    if let Ok(mut pending) = state.pending_actions.lock() {
                                        pending.insert(
                                            target_name.clone(),
                                            "restarting...".to_string(),
                                        );
                                    }
                                    state.status_msg = format!("⟳ restarting {}...", target_name);

                                    let cp_clone = cp.clone();
                                    let cfg_clone = cfg.clone();
                                    let target_clone = target_name.clone();
                                    let pending_clone = Arc::clone(&state.pending_actions);

                                    thread::spawn(move || {
                                        let _ = process_manager::restart(
                                            &cp_clone,
                                            &cfg_clone,
                                            Some(&target_clone),
                                            false,
                                        );
                                        if let Ok(mut p) = pending_clone.lock() {
                                            p.remove(&target_clone);
                                        }
                                    });
                                }
                            }
                        }
                        KeyCode::Char('x') => {
                            if state.is_global_mode {
                                if let Some(g) = state.global_rows.get(state.selected_idx).cloned()
                                {
                                    let key = format!("{}/{}", g.project_key, g.service_name);
                                    if let Ok(mut pending) = state.pending_actions.lock() {
                                        pending.insert(key, "stopping...".to_string());
                                    }
                                    state.status_msg = format!(
                                        "⟳ stopping {} in {}...",
                                        g.service_name, g.project_key
                                    );

                                    let cp_opt = g.config_path.clone();
                                    let service_clone = g.service_name.clone();
                                    let pid_opt = g.pid;
                                    let pending_clone = Arc::clone(&state.pending_actions);
                                    let key_clone = format!("{}/{}", g.project_key, g.service_name);

                                    thread::spawn(move || {
                                        if let Some(cp) = cp_opt {
                                            if let Ok(cfg) = crate::config::load_config(&cp) {
                                                let _ = process_manager::stop(
                                                    &cp,
                                                    &cfg,
                                                    Some(&service_clone),
                                                );
                                            } else if let Some(pid) = pid_opt {
                                                let _ = stop_by_pid(pid);
                                            }
                                        } else if let Some(pid) = pid_opt {
                                            let _ = stop_by_pid(pid);
                                        }
                                        if let Ok(mut p) = pending_clone.lock() {
                                            p.remove(&key_clone);
                                        }
                                    });
                                }
                            } else if let Some((ref cp, ref cfg)) = local_config_opt {
                                if let Some(target_name) = state
                                    .local_rows
                                    .get(state.selected_idx)
                                    .map(|r| r.name.clone())
                                {
                                    if let Ok(mut pending) = state.pending_actions.lock() {
                                        pending
                                            .insert(target_name.clone(), "stopping...".to_string());
                                    }
                                    state.status_msg = format!("⟳ stopping {}...", target_name);

                                    let cp_clone = cp.clone();
                                    let cfg_clone = cfg.clone();
                                    let target_clone = target_name.clone();
                                    let pending_clone = Arc::clone(&state.pending_actions);

                                    thread::spawn(move || {
                                        let _ = process_manager::stop(
                                            &cp_clone,
                                            &cfg_clone,
                                            Some(&target_clone),
                                        );
                                        if let Ok(mut p) = pending_clone.lock() {
                                            p.remove(&target_clone);
                                        }
                                    });
                                }
                            }
                        }
                        KeyCode::Char('k') | KeyCode::Char('K') => {
                            if state.is_global_mode {
                                if let Some(g) = state.global_rows.get(state.selected_idx).cloned()
                                {
                                    let key = format!("{}/{}", g.project_key, g.service_name);
                                    if let Ok(mut pending) = state.pending_actions.lock() {
                                        pending.insert(key, "killing...".to_string());
                                    }
                                    state.status_msg = format!(
                                        "🛑 force-killing {} in {}...",
                                        g.service_name, g.project_key
                                    );

                                    let cp_opt = g.config_path.clone();
                                    let service_clone = g.service_name.clone();
                                    let pid_opt = g.pid;
                                    let port_opt = g.port;
                                    let pending_clone = Arc::clone(&state.pending_actions);
                                    let key_clone = format!("{}/{}", g.project_key, g.service_name);

                                    thread::spawn(move || {
                                        if let Some(cp) = cp_opt {
                                            if let Ok(cfg) = crate::config::load_config(&cp) {
                                                let _ = process_manager::force_stop(
                                                    &cp,
                                                    &cfg,
                                                    Some(&service_clone),
                                                );
                                            } else if let Some(pid) = pid_opt {
                                                let _ = force_kill_by_pid(pid);
                                                if let Some(port) = port_opt {
                                                    process_manager::kill_port(port);
                                                }
                                            }
                                        } else if let Some(pid) = pid_opt {
                                            let _ = force_kill_by_pid(pid);
                                            if let Some(port) = port_opt {
                                                process_manager::kill_port(port);
                                            }
                                        }
                                        if let Ok(mut p) = pending_clone.lock() {
                                            p.remove(&key_clone);
                                        }
                                    });
                                }
                            } else if let Some((ref cp, ref cfg)) = local_config_opt {
                                if let Some(target_name) = state
                                    .local_rows
                                    .get(state.selected_idx)
                                    .map(|r| r.name.clone())
                                {
                                    if let Ok(mut pending) = state.pending_actions.lock() {
                                        pending
                                            .insert(target_name.clone(), "killing...".to_string());
                                    }
                                    state.status_msg =
                                        format!("🛑 force-killing {}...", target_name);

                                    let cp_clone = cp.clone();
                                    let cfg_clone = cfg.clone();
                                    let target_clone = target_name.clone();
                                    let pending_clone = Arc::clone(&state.pending_actions);

                                    thread::spawn(move || {
                                        let _ = process_manager::force_stop(
                                            &cp_clone,
                                            &cfg_clone,
                                            Some(&target_clone),
                                        );
                                        if let Ok(mut p) = pending_clone.lock() {
                                            p.remove(&target_clone);
                                        }
                                    });
                                }
                            }
                        }
                        KeyCode::Char('f') => {
                            if state.is_global_mode {
                                if let Some(g) = state.global_rows.get(state.selected_idx).cloned()
                                {
                                    if let Some(ref cp) = g.config_path {
                                        if let Ok(cfg) = crate::config::load_config(cp) {
                                            match forward_start(cp, &cfg, &g.service_name) {
                                                Ok((url, _)) => {
                                                    state.status_msg =
                                                        format!("{} -> {}", g.service_name, url);
                                                }
                                                Err(e) => {
                                                    state.status_msg =
                                                        format!("forward failed: {}", e);
                                                }
                                            }
                                        }
                                    }
                                }
                            } else if let Some((ref cp, ref cfg)) = local_config_opt {
                                if let Some(target_name) = state
                                    .local_rows
                                    .get(state.selected_idx)
                                    .map(|r| r.name.clone())
                                {
                                    match forward_start(cp, cfg, &target_name) {
                                        Ok((url, _)) => {
                                            state.status_msg =
                                                format!("{} -> {}", target_name, url);
                                        }
                                        Err(e) => {
                                            state.status_msg = format!("forward failed: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                        KeyCode::Char('u') => {
                            if state.is_global_mode {
                                if let Some(g) = state.global_rows.get(state.selected_idx).cloned()
                                {
                                    if let Some(ref cp) = g.config_path {
                                        match forward_stop(cp, &g.service_name) {
                                            Ok(stopped) => {
                                                state.status_msg = if stopped {
                                                    format!("tunnel stopped for {}", g.service_name)
                                                } else {
                                                    format!("no tunnel for {}", g.service_name)
                                                };
                                            }
                                            Err(e) => {
                                                state.status_msg =
                                                    format!("error stopping tunnel: {}", e);
                                            }
                                        }
                                    }
                                }
                            } else if let Some((ref cp, _)) = local_config_opt {
                                if let Some(target_name) = state
                                    .local_rows
                                    .get(state.selected_idx)
                                    .map(|r| r.name.clone())
                                {
                                    match forward_stop(cp, &target_name) {
                                        Ok(stopped) => {
                                            state.status_msg = if stopped {
                                                format!("tunnel stopped for {}", target_name)
                                            } else {
                                                format!("no tunnel for {}", target_name)
                                            };
                                        }
                                        Err(e) => {
                                            state.status_msg =
                                                format!("error stopping tunnel: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                        KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete => {
                            if state.is_global_mode {
                                if let Some(g) = state.global_rows.get(state.selected_idx).cloned()
                                {
                                    let _ = crate::registry::ProjectRegistry::unregister(
                                        &g.project_key,
                                    );
                                    let local_ref =
                                        local_config_opt.as_ref().map(|(p, c)| (p.as_path(), c));
                                    state.refresh_rows(local_ref);
                                    state.status_msg = format!(
                                        "unregistered project \"{}\" from dashboard",
                                        g.project_key
                                    );
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    Ok(())
}
