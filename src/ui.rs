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
    Terminal,
};
use std::io::stdout;
use std::time::{Duration, Instant};

use crate::cloudflare::{forward_start, forward_stop};
use crate::config::require_config;
use crate::logs::read_tail;
use crate::process_manager::{self, log_file_for};
use crate::qr::render_qr;

const NORMAL_LOG_LINES: usize = 18;
const FULLSCREEN_LOG_LINES: usize = 500;
const UI_REFRESH_INTERVAL: Duration = Duration::from_millis(1000);
const EVENT_POLL_TIMEOUT: Duration = Duration::from_millis(150);
const DOT_RUNNING: &str = "●";
const DOT_STOPPED: &str = "○";
const DEFAULT_PLACEHOLDER: &str = "-";

pub fn render_ui() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal);

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

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>) -> Result<()> {
    let (config_path, config) = require_config()?;
    let mut names: Vec<String> = config.processes.keys().cloned().collect();
    names.sort();

    let mut selected_idx: usize = 0;
    let mut status_msg = "ready".to_string();
    let mut rows = process_manager::status(&config_path, &config, None).unwrap_or_default();
    let mut last_refresh = Instant::now();
    let mut show_qr_modal = false;
    let mut is_fullscreen_log = false;
    let mut is_search_input = false;
    let mut search_query = String::new();
    let mut scroll_offset: usize = 0;

    loop {
        if last_refresh.elapsed() >= UI_REFRESH_INTERVAL {
            rows = process_manager::status(&config_path, &config, None).unwrap_or_default();
            last_refresh = Instant::now();
        }

        let selected_name = names.get(selected_idx).cloned().unwrap_or_default();
        let selected_row = rows.iter().find(|r| r.name == selected_name).cloned();

        let log_file = log_file_for(&config_path, &selected_name)?;
        let lines_to_read = if is_fullscreen_log {
            FULLSCREEN_LOG_LINES
        } else {
            NORMAL_LOG_LINES
        };
        let raw_tail = read_tail(&log_file, lines_to_read);

        let filtered_lines: Vec<&str> = if search_query.is_empty() {
            raw_tail.lines().collect()
        } else {
            let lower_query = search_query.to_lowercase();
            raw_tail
                .lines()
                .filter(|l| l.to_lowercase().contains(&lower_query))
                .collect()
        };

        let total_log_count = filtered_lines.len();

        let visible_slice = if scroll_offset > 0 && total_log_count > scroll_offset {
            let end = total_log_count - scroll_offset;
            &filtered_lines[..end]
        } else {
            &filtered_lines[..]
        };

        let styled_log_lines: Vec<Line> = visible_slice
            .iter()
            .map(|l| highlight_line(l, &search_query))
            .collect();

        terminal.draw(|f| {
            let area = f.area();

            if is_fullscreen_log {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Min(8),
                        Constraint::Length(1),
                        Constraint::Length(1),
                    ])
                    .split(area);

                let search_badge = if !search_query.is_empty() {
                    format!(" [Filter: \"{}\" ({} matches)]", search_query, total_log_count)
                } else {
                    String::new()
                };
                let scroll_badge = if scroll_offset > 0 {
                    format!(" [Scrolled +{}]", scroll_offset)
                } else {
                    String::new()
                };

                let log_title = format!(
                    " FULLSCREEN LOG: {} {}{}- (Enter/Esc to exit) ",
                    selected_name, search_badge, scroll_badge
                );
                let log_paragraph = Paragraph::new(styled_log_lines).block(
                    Block::default()
                        .title(log_title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)),
                );
                f.render_widget(log_paragraph, chunks[0]);

                let help_spans = if is_search_input {
                    vec![
                        Span::styled("Search: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::styled(format!("{}_", search_query), Style::default().fg(Color::White)),
                        Span::raw("  (Enter to filter, Esc to cancel)"),
                    ]
                } else {
                    vec![
                        Span::styled("Enter/Space", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::raw(" exit fullscreen  "),
                        Span::styled("↑/↓/PgUp/PgDn", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::raw(" scroll  "),
                        Span::styled("G", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                        Span::raw(" latest  "),
                        Span::styled("/", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                        Span::raw(" search  "),
                        Span::styled("c", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                        Span::raw(" clear filter  "),
                        Span::styled("q", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
                        Span::raw(" quit"),
                    ]
                };
                f.render_widget(Paragraph::new(Line::from(help_spans)), chunks[1]);

                let msg_line = Line::from(vec![
                    Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        status_msg.clone(),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]);
                f.render_widget(Paragraph::new(msg_line), chunks[2]);
            } else {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Length((names.len() as u16 + 3).max(5)),
                        Constraint::Min(8),
                        Constraint::Length(1),
                        Constraint::Length(1),
                    ])
                    .split(area);

                let table_rows: Vec<Row> = rows
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        let is_selected = i == selected_idx;
                        let dot = if r.running { DOT_RUNNING } else { DOT_STOPPED };
                        let dot_color = if r.running { Color::Green } else { Color::Red };

                        let pid_str = r.pid.map(|p| p.to_string()).unwrap_or_else(|| DEFAULT_PLACEHOLDER.into());
                        let cpu_str = r.cpu.map(|c| format!("{:.1}%", c)).unwrap_or_else(|| DEFAULT_PLACEHOLDER.into());
                        let mem_str = r.memory_mb.map(|m| format!("{}MB", m)).unwrap_or_else(|| DEFAULT_PLACEHOLDER.into());
                        let up_str = r.uptime.clone().unwrap_or_else(|| DEFAULT_PLACEHOLDER.into());
                        let port_str = r.port.map(|p| p.to_string()).unwrap_or_else(|| DEFAULT_PLACEHOLDER.into());
                        let tunnel_str = r.tunnel_url.clone().unwrap_or_default();

                        let row = Row::new(vec![
                            Line::from(vec![
                                Span::styled(format!("{} ", dot), Style::default().fg(dot_color)),
                                Span::raw(r.name.clone()),
                            ]),
                            Line::from(pid_str),
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

                let header = Row::new(vec!["NAME", "PID", "CPU", "MEM", "UPTIME", "PORT", "TUNNEL"])
                    .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

                let table = Table::new(
                    table_rows,
                    [
                        Constraint::Percentage(22),
                        Constraint::Percentage(9),
                        Constraint::Percentage(9),
                        Constraint::Percentage(9),
                        Constraint::Percentage(13),
                        Constraint::Percentage(9),
                        Constraint::Percentage(29),
                    ],
                )
                .header(header);
                let update_badge = if let Some(latest) = &crate::updater::UpdateCache::load().latest_version {
                    if crate::updater::is_newer(crate::updater::CURRENT_VERSION, latest) {
                        format!(" [🔔 v{} available]", latest)
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                let table = table.block(
                    Block::default()
                        .title(format!(" Processes ({}){} ", config_path.display(), update_badge))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Gray)),
                );

                f.render_widget(table, chunks[0]);

                let search_badge = if !search_query.is_empty() {
                    format!(" [Filter: \"{}\" ({} matches)]", search_query, total_log_count)
                } else {
                    String::new()
                };

                let log_title = format!(" log: {} {} (Enter: fullscreen, /: search) ", selected_name, search_badge);
                let log_paragraph = Paragraph::new(styled_log_lines).block(
                    Block::default()
                        .title(log_title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)),
                );
                f.render_widget(log_paragraph, chunks[1]);

                let help_spans = if is_search_input {
                    vec![
                        Span::styled("Search: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::styled(format!("{}_", search_query), Style::default().fg(Color::White)),
                        Span::raw("  (Enter to filter, Esc to cancel)"),
                    ]
                } else {
                    vec![
                        Span::styled("↑/↓", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                        Span::raw(" select  "),
                        Span::styled("Enter", Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD)),
                        Span::raw(" zoom log  "),
                        Span::styled("/", Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD)),
                        Span::raw(" search  "),
                        Span::styled("s", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                        Span::raw(" start  "),
                        Span::styled("x", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                        Span::raw(" stop  "),
                        Span::styled("k", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                        Span::raw(" kill  "),
                        Span::styled("r", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                        Span::raw(" restart  "),
                        Span::styled("f", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                        Span::raw(" forward  "),
                        Span::styled("o", Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)),
                        Span::raw(" QR  "),
                        Span::styled("q", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
                        Span::raw(" quit"),
                    ]
                };
                f.render_widget(Paragraph::new(Line::from(help_spans)), chunks[2]);

                let msg_line = Line::from(vec![
                    Span::styled("Status: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        status_msg.clone(),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]);
                f.render_widget(Paragraph::new(msg_line), chunks[3]);
            }

            if show_qr_modal {
                let target_url = selected_row
                    .as_ref()
                    .and_then(|r| r.tunnel_url.clone())
                    .or_else(|| {
                        selected_row
                            .as_ref()
                            .and_then(|r| r.port)
                            .map(|p| format!("http://localhost:{}", p))
                    });

                if let Some(url) = target_url {
                    let qr_code_str = render_qr(&url).unwrap_or_else(|| "Failed to generate QR".to_string());
                    let popup_area = centered_rect(70, 80, area);

                    f.render_widget(Clear, popup_area);

                    let mut qr_lines: Vec<Line> = qr_code_str
                        .lines()
                        .map(|l| Line::from(Span::raw(l.to_string())))
                        .collect();

                    qr_lines.insert(
                        0,
                        Line::from(vec![
                            Span::styled("Scan with Mobile Camera: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                            Span::styled(&url, Style::default().fg(Color::Yellow)),
                        ]),
                    );
                    qr_lines.insert(1, Line::from(""));
                    qr_lines.push(Line::from(""));
                    qr_lines.push(Line::from(Span::styled(
                        "Press 'o' or 'Esc' to close",
                        Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
                    )));

                    let popup_block = Block::default()
                        .title(format!(" QR Code: {} ", selected_name))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Green));

                    let popup_paragraph = Paragraph::new(qr_lines)
                        .block(popup_block)
                        .alignment(Alignment::Center);

                    f.render_widget(popup_paragraph, popup_area);
                }
            }
        })?;

        if event::poll(EVENT_POLL_TIMEOUT)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if show_qr_modal {
                        match key.code {
                            KeyCode::Char('o') | KeyCode::Char('O') | KeyCode::Char('q') | KeyCode::Esc => {
                                show_qr_modal = false;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    if is_search_input {
                        match key.code {
                            KeyCode::Esc => {
                                is_search_input = false;
                                search_query.clear();
                                scroll_offset = 0;
                            }
                            KeyCode::Enter => {
                                is_search_input = false;
                                scroll_offset = 0;
                                status_msg = if search_query.is_empty() {
                                    "search filter cleared".to_string()
                                } else {
                                    format!("filter applied: \"{}\"", search_query)
                                };
                            }
                            KeyCode::Backspace => {
                                search_query.pop();
                            }
                            KeyCode::Char(c) => {
                                search_query.push(c);
                            }
                            _ => {}
                        }
                        continue;
                    }

                    if is_fullscreen_log {
                        match key.code {
                            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Esc => {
                                is_fullscreen_log = false;
                                scroll_offset = 0;
                            }
                            KeyCode::Char('/') => {
                                is_search_input = true;
                            }
                            KeyCode::Char('c') | KeyCode::Char('C') => {
                                search_query.clear();
                                scroll_offset = 0;
                                status_msg = "filter cleared".to_string();
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                scroll_offset = scroll_offset.saturating_add(1);
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                scroll_offset = scroll_offset.saturating_sub(1);
                            }
                            KeyCode::PageUp => {
                                scroll_offset = scroll_offset.saturating_add(15);
                            }
                            KeyCode::PageDown => {
                                scroll_offset = scroll_offset.saturating_sub(15);
                            }
                            KeyCode::Home | KeyCode::Char('g') => {
                                scroll_offset = total_log_count.saturating_sub(10);
                            }
                            KeyCode::End | KeyCode::Char('G') => {
                                scroll_offset = 0;
                            }
                            KeyCode::Char('q') => break,
                            _ => {}
                        }
                        continue;
                    }

                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Enter | KeyCode::Char(' ') => {
                            is_fullscreen_log = true;
                            scroll_offset = 0;
                        }
                        KeyCode::Char('/') => {
                            is_search_input = true;
                        }
                        KeyCode::Char('c') | KeyCode::Char('C') => {
                            search_query.clear();
                            scroll_offset = 0;
                            status_msg = "filter cleared".to_string();
                        }
                        KeyCode::Char('o') | KeyCode::Char('O') => {
                            if let Some(row) = selected_row.as_ref() {
                                if row.tunnel_url.is_some() || row.port.is_some() {
                                    show_qr_modal = !show_qr_modal;
                                } else {
                                    status_msg = format!("no tunnel URL or port for {}", selected_name);
                                }
                            }
                        }
                        KeyCode::Up => {
                            if selected_idx > 0 {
                                selected_idx -= 1;
                            } else if !names.is_empty() {
                                selected_idx = names.len() - 1;
                            }
                        }
                        KeyCode::Down => {
                            if !names.is_empty() {
                                selected_idx = (selected_idx + 1) % names.len();
                            }
                        }
                        KeyCode::Char('s') => {
                            if let Some(target) = names.get(selected_idx) {
                                let _ = process_manager::start(&config_path, &config, Some(target), false);
                                rows = process_manager::status(&config_path, &config, None).unwrap_or_default();
                                status_msg = format!("{} started", target);
                            }
                        }
                        KeyCode::Char('x') => {
                            if let Some(target) = names.get(selected_idx) {
                                let _ = process_manager::stop(&config_path, &config, Some(target));
                                rows = process_manager::status(&config_path, &config, None).unwrap_or_default();
                                status_msg = format!("{} stopped", target);
                            }
                        }
                        KeyCode::Char('k') | KeyCode::Char('K') => {
                            if let Some(target) = names.get(selected_idx) {
                                let _ = process_manager::force_stop(&config_path, &config, Some(target));
                                rows = process_manager::status(&config_path, &config, None).unwrap_or_default();
                                status_msg = format!("{} force-killed & port freed", target);
                            }
                        }
                        KeyCode::Char('r') => {
                            if let Some(target) = names.get(selected_idx) {
                                let _ = process_manager::restart(&config_path, &config, Some(target), false);
                                rows = process_manager::status(&config_path, &config, None).unwrap_or_default();
                                status_msg = format!("{} restarted", target);
                            }
                        }
                        KeyCode::Char('f') => {
                            if let Some(target) = names.get(selected_idx) {
                                match forward_start(&config_path, &config, target) {
                                    Ok((url, _)) => {
                                        rows = process_manager::status(&config_path, &config, None).unwrap_or_default();
                                        status_msg = format!("{} -> {}", target, url);
                                    }
                                    Err(e) => {
                                        status_msg = format!("forward failed: {}", e);
                                    }
                                }
                            }
                        }
                        KeyCode::Char('u') => {
                            if let Some(target) = names.get(selected_idx) {
                                match forward_stop(&config_path, target) {
                                    Ok(stopped) => {
                                        rows = process_manager::status(&config_path, &config, None).unwrap_or_default();
                                        status_msg = if stopped {
                                            format!("tunnel stopped for {}", target)
                                        } else {
                                            format!("no tunnel for {}", target)
                                        };
                                    }
                                    Err(e) => {
                                        status_msg = format!("error stopping tunnel: {}", e);
                                    }
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
