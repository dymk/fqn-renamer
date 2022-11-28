use crate::{app::App, ui};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use std::error::Error;
use tui::{
    backend::Backend, interactive_form::InteractiveForm, widgets::InteractiveWidgetState, Terminal,
};

pub enum AppEvent {
    Crossterm(crossterm::event::Event),
    Redraw,
    WorkerUpdate,
    Abort(String),
}

pub fn handle_event<B: Backend>(
    event: AppEvent,
    app: &mut App,
    terminal: &mut Terminal<B>,
) -> Result<bool, Box<dyn Error>> {
    let should_continue = match event {
        AppEvent::Crossterm(event) => handle_crossterm_event(event, app),
        AppEvent::Redraw => Ok(true),
        AppEvent::WorkerUpdate => {
            app.search_worker_finished();
            Ok(true)
        }
        AppEvent::Abort(str) => {
            app.events.error(format!("app: abort: {}", str));
            Ok(false)
        }
    }?;

    if should_continue {
        terminal.draw(|f| ui::draw(f, app))?;
    }

    Ok(should_continue)
}

fn handle_crossterm_event(
    event: crossterm::event::Event,
    app: &mut App,
) -> Result<bool, Box<dyn Error>> {
    if let Event::Key(KeyEvent {
        code: KeyCode::Enter,
        modifiers: KeyModifiers::NONE,
    }) = event
    {
        // 'enter' key pressed while on search / searching... button => toggle search
        if app.inputs.search_button.is_focused() {
            app.search_button_submitted();
        }
        // 'enter' key pressed while on search input => start search
        else if app.inputs.search_for_ident.is_focused() {
            app.search_input_submitted();
        } else if app.inputs.replace_button.is_focused() {
            app.replace_input_submitted();
        }
    }

    if let Event::Key(KeyEvent {
        code: KeyCode::Up,
        modifiers: KeyModifiers::NONE,
    }) = event
    {
        app.results_scroll_offset = app.results_scroll_offset.saturating_sub(1);
    }

    if let Event::Key(KeyEvent {
        code: KeyCode::Down,
        modifiers: KeyModifiers::NONE,
    }) = event
    {
        app.results_scroll_offset = app.results_scroll_offset.saturating_add(1);
    }

    if let Event::Key(KeyEvent {
        code: KeyCode::PageUp,
        modifiers: KeyModifiers::NONE,
    }) = event
    {
        app.results_scroll_offset = app.results_scroll_offset.saturating_sub(10);
    }

    if let Event::Key(KeyEvent {
        code: KeyCode::PageDown,
        modifiers: KeyModifiers::NONE,
    }) = event
    {
        app.results_scroll_offset = app.results_scroll_offset.saturating_add(10);
    }

    let consumed = app.inputs.handle_event(event).is_consumed();

    if app.inputs.replace_with_ident.changed() {
        app.update_replacements();
    }

    if consumed {
        return Ok(true);
    }

    if let Event::Key(key_event) = event {
        match key_event {
            // scrolling
            KeyEvent {
                code: KeyCode::Up,
                modifiers: KeyModifiers::NONE,
            } => {
                app.results_scroll_offset = app.results_scroll_offset.saturating_sub(1);
            }

            KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
            } => {
                app.results_scroll_offset = app.results_scroll_offset.saturating_add(1);
            }

            KeyEvent {
                code: KeyCode::PageUp,
                modifiers: KeyModifiers::NONE,
            } => {
                app.results_scroll_offset = app.results_scroll_offset.saturating_sub(10);
            }

            KeyEvent {
                code: KeyCode::PageDown,
                modifiers: KeyModifiers::NONE,
            } => {
                app.results_scroll_offset = app.results_scroll_offset.saturating_add(10);
            }

            // event log visibility
            KeyEvent {
                code: KeyCode::Char('l'),
                modifiers: KeyModifiers::CONTROL,
            } => {
                app.show_events = !app.show_events;
            }

            // quit the app
            KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE,
            }
            | KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
            } => return Ok(false),

            // input navigation
            KeyEvent {
                code: KeyCode::Tab,
                modifiers: KeyModifiers::NONE,
            } => app.inputs.focus_next_input(),

            KeyEvent {
                code: KeyCode::BackTab,
                modifiers: KeyModifiers::SHIFT,
            } => app.inputs.focus_prev_input(),
            _ => {}
        }
    }

    Ok(true)
}
