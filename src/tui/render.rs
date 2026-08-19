use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table},
    Frame,
};

use super::app::AppState;
use crate::tunnels::qr::render_qr;

const DOT_RUNNING: &str = "●";
const DOT_STOPPED: &str = "○";
const DOT_PENDING: &str = "⟳";
const DEFAULT_PLACEHOLDER: &str = "-";

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

pub fn highlight_line<'a>(line: &'a str, query: &str) -> Line<'a> {
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

pub fn render_global_table(f: &mut Frame, area: Rect, state: &AppState) {
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

pub fn render_local_table(f: &mut Frame, area: Rect, state: &AppState) {
    let pending_guard = state.pending_actions.lock().ok();
    let config_path_display = state
        .local_config
        .as_ref()
        .map(|(p, _)| p.display().to_string())
        .unwrap_or_default();

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
                config_path_display
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray)),
    );

    f.render_widget(table, area);
}

pub fn build_help_spans(state: &AppState) -> Vec<Span<'static>> {
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

pub fn render_qr_popup(f: &mut Frame, area: Rect, title: &str, url: &str) {
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

pub fn draw(f: &mut Frame, state: &AppState) {
    let area = f.area();

    let filtered_lines: Vec<&str> = if state.search_query.is_empty() {
        state.log_lines.iter().map(|s| s.as_str()).collect()
    } else {
        let lower_query = state.search_query.to_lowercase();
        state
            .log_lines
            .iter()
            .map(|s| s.as_str())
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
            mode_badge, state.display_title, search_badge, scroll_badge
        );
        let log_paragraph = Paragraph::new(styled_log_lines).block(
            Block::default()
                .title(log_title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );
        f.render_widget(log_paragraph, chunks[0]);

        let help_spans = build_help_spans(state);
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
        let table_height = (state.row_count as u16 + 3).max(5).min(area.height / 2);
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
            render_global_table(f, chunks[0], state);
        } else {
            render_local_table(f, chunks[0], state);
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
            state.display_title, search_badge
        );
        let log_paragraph = Paragraph::new(styled_log_lines).block(
            Block::default()
                .title(log_title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );
        f.render_widget(log_paragraph, chunks[1]);

        let help_spans = build_help_spans(state);
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
        if let Some(ref url) = state.selected_url {
            render_qr_popup(f, area, &state.display_title, url);
        }
    }
}
