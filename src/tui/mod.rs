pub mod actions;
pub mod app;
pub mod events;
pub mod logview;
pub mod render;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
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
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, start_all);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
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
            state.dirty = true;
        }
        state.clamp_selected_idx();
        state.refresh_log_view();

        if state.dirty {
            terminal.draw(|f| render::draw(f, &mut state))?;
            state.dirty = false;
        }

        // Wait briefly for first event, then drain the rest so a burst of
        // mouse-wheel events costs a single redraw.
        let mut timeout = EVENT_POLL_TIMEOUT;
        while event::poll(timeout)? {
            timeout = Duration::ZERO;
            state.dirty = true;
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press
                        && events::handle_key(&mut state, key.code).is_break()
                    {
                        return Ok(());
                    }
                }
                Event::Mouse(mouse) => events::handle_mouse(&mut state, mouse),
                _ => {}
            }
        }
    }
}
