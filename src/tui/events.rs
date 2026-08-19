use crossterm::event::KeyCode;

use super::actions;
use super::app::AppState;

pub enum KeyActionResult {
    Continue,
    Break,
}

impl KeyActionResult {
    pub fn is_break(&self) -> bool {
        matches!(self, KeyActionResult::Break)
    }
}

pub fn handle_key(state: &mut AppState, code: KeyCode) -> KeyActionResult {
    if state.show_qr_modal {
        match code {
            KeyCode::Char('o') | KeyCode::Char('O') | KeyCode::Char('q') | KeyCode::Esc => {
                state.show_qr_modal = false;
            }
            _ => {}
        }
        return KeyActionResult::Continue;
    }

    if state.is_search_input {
        match code {
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
        return KeyActionResult::Continue;
    }

    if state.is_fullscreen_log {
        let total_log_count = state.log_lines.len();
        match code {
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
                state.scroll_offset = state.scroll_offset.saturating_add(15);
            }
            KeyCode::PageDown => {
                state.scroll_offset = state.scroll_offset.saturating_sub(15);
            }
            KeyCode::Home | KeyCode::Char('g') => {
                state.scroll_offset = total_log_count.saturating_sub(10);
            }
            KeyCode::End | KeyCode::Char('G') => {
                state.scroll_offset = 0;
            }
            KeyCode::Char('q') => return KeyActionResult::Break,
            _ => {}
        }
        return KeyActionResult::Continue;
    }

    match code {
        KeyCode::Tab => {
            if state.local_config.is_some() {
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
                state.status_msg = "No local procman.yaml (staying in Global View)".to_string();
            }
        }
        KeyCode::Char('q') | KeyCode::Esc => return KeyActionResult::Break,
        KeyCode::Enter | KeyCode::Char(' ') => {
            if state.row_count > 0 {
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
            if state.selected_url.is_some() {
                state.show_qr_modal = !state.show_qr_modal;
            } else {
                state.status_msg = format!("no tunnel URL or port for {}", state.display_title);
            }
        }
        KeyCode::Up => {
            if state.selected_idx > 0 {
                state.selected_idx -= 1;
            } else if state.row_count > 0 {
                state.selected_idx = state.row_count - 1;
            }
        }
        KeyCode::Down => {
            if state.row_count > 0 {
                state.selected_idx = (state.selected_idx + 1) % state.row_count;
            }
        }
        KeyCode::Char('s') => actions::start_selected(state),
        KeyCode::Char('r') => actions::restart_selected(state),
        KeyCode::Char('x') => actions::stop_selected(state),
        KeyCode::Char('k') | KeyCode::Char('K') => actions::force_kill_selected(state),
        KeyCode::Char('f') => actions::forward_selected(state),
        KeyCode::Char('u') => actions::unforward_selected(state),
        KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete => {
            actions::unregister_selected(state)
        }
        _ => {}
    }

    KeyActionResult::Continue
}
