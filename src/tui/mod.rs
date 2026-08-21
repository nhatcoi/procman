pub mod actions;
pub mod app;
pub mod events;
pub mod render;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::stdout;
use std::time::Duration;

use crate::engine::config::{find_config_path, load_config};
use app::AppState;

const EVENT_POLL_TIMEOUT: Duration = Duration::from_millis(50);

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

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    start_all: bool,
) -> Result<()> {
    let local_config = find_config_path(None).and_then(|p| {
        let _ = crate::engine::registry::ProjectRegistry::register(&p);
        load_config(&p).ok().map(|cfg| (p, cfg))
    });

    let mut state = AppState::new(start_all, local_config);

    loop {
        if state.needs_refresh() {
            state.refresh_rows();
        }
        state.clamp_selected_idx();
        state.refresh_log_view();

        terminal.draw(|f| render::draw(f, &state))?;

        if event::poll(EVENT_POLL_TIMEOUT)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press
                    && events::handle_key(&mut state, key.code).is_break()
                {
                    break;
                }
            }
        }
    }

    Ok(())
}
