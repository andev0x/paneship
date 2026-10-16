//! Embedded TUI configurator (Phase 2, Module C).
//!
//! Launched by `paneship config`. Uses `ratatui` + `crossterm` to provide a
//! low-latency split-pane editor: sections on the left, a live prompt preview
//! and the section's fields on the right. Saving writes atomically and signals
//! the daemon to hot-reload.

pub mod app;
pub mod persist;
pub mod ui;

use app::{Action, App};
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;
use std::time::Duration;

/// Run the interactive configurator to completion.
pub fn run() -> Result<(), String> {
    let mut app = App::load()?;
    let mut terminal =
        ratatui::try_init().map_err(|err| format!("failed to initialise terminal: {err}"))?;

    let result = event_loop(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut DefaultTerminal, app: &mut App) -> Result<(), String> {
    // ~150 ms poll keeps the UI responsive to keys while idling cheaply.
    loop {
        terminal
            .draw(|frame| ui::draw(frame, app))
            .map_err(|err| err.to_string())?;

        if !event::poll(Duration::from_millis(150)).map_err(|err| err.to_string())? {
            continue;
        }

        match event::read().map_err(|err| err.to_string())? {
            Event::Key(key) if key.kind == KeyEventKind::Press => match app.handle_key(key) {
                Action::Quit => return Ok(()),
                Action::Save => {
                    if let Err(err) = app.save() {
                        app.status = format!("Save failed: {err}");
                    }
                }
                Action::None => {}
            },
            // A terminal resize (SIGWINCH on Unix) arrives here; the next draw
            // pass re-reads `frame.area()` and reflows the layout for narrow
            // tmux split panes.
            Event::Resize(_, _) => {}
            _ => {}
        }
    }
}
