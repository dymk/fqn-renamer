mod app;
mod event_log;

use app::{App, FoundMatch};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{env, error::Error, io, time::Duration};
use tui::{
    backend::{Backend, CrosstermBackend},
    interactive_form::InteractiveForm,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, List, ListItem, TextInput},
    Frame, Terminal,
};

fn main() -> Result<(), Box<dyn Error>> {
    // main argument parsing
    let base_dir = env::args().next().expect("arg0: base dir to search");

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let mut app = App::new(base_dir);
    let res = run_app(&mut terminal, &mut app);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;
        app.check_search_done();

        let has_event = event::poll(Duration::from_millis(100))?;
        if !has_event {
            continue;
        }

        let event = event::read()?;

        if !matches!(event, Event::Mouse(_)) {
            app.events
                .lock()
                .unwrap()
                .push(format!("term event: {:?}", event));
        }

        if let Event::Key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
        }) = event
        {
            if app.inputs.search_button.is_focused() {
                app.search_button_submitted();
            }
        }

        if app.inputs.handle_event(event).is_consumed() {
            continue;
        }

        match event {
            Event::Key(key) => match key.code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Tab => app.inputs.focus_next_input(),
                KeyCode::BackTab => app.inputs.focus_prev_input(),
                _ => {}
            },
            _ => {}
        }
    }
}

fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let layout = Layout::default()
        .horizontal_margin(10)
        .vertical_margin(2)
        .constraints(
            [
                // inputs
                Constraint::Length(6),
                // results
                Constraint::Min(10),
                // event log
                Constraint::Length(15),
            ]
            .as_ref(),
        )
        .split(f.size());

    let inputs_layout = Layout::default()
        .constraints([Constraint::Length(3), Constraint::Length(3)].as_ref())
        .split(layout[0]);

    let default_block = || {
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray))
    };
    let focused_style = || {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    };

    // "Search" input and preview button
    {
        let l = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(10), Constraint::Length(20)].as_ref())
            .split(inputs_layout[0]);

        let search_input = TextInput::new()
            .block(default_block().title("Search").borders(Borders::ALL))
            .focused_style(focused_style())
            .placeholder_text("Identifier or FQCN");

        f.render_interactive(search_input, l[0], &app.inputs.search_for_ident);

        let preview_button = TextInput::new()
            .disable_cursor(true)
            .alignment(tui::layout::Alignment::Center)
            .focused_style(focused_style())
            .block(default_block().borders(Borders::ALL));
        f.render_interactive(preview_button, l[1], &app.inputs.search_button)
    }

    // "Replace" input and preview button
    {
        let l = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(10), Constraint::Length(20)].as_ref())
            .split(inputs_layout[1]);

        let search_input = TextInput::new()
            .focused_style(focused_style())
            .block(default_block().title("Replace").borders(Borders::ALL))
            .placeholder_text("Identifier or FQCN");

        f.render_interactive(search_input, l[0], &app.inputs.replace_with_ident);

        let replace_button = TextInput::new()
            .focused_style(focused_style())
            .disable_cursor(true)
            .alignment(tui::layout::Alignment::Center)
            .block(default_block().borders(Borders::ALL));
        f.render_interactive(replace_button, l[1], &app.inputs.replace_button)
    }

    // Results / Replacement Preview area
    {
        let l = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(layout[1]);
        let search_results_l = l[0];
        let replace_review_l = l[1];

        let matches = app.found_matches.lock().unwrap();
        let search_results = List::new(
            matches
                .iter()
                .map(|found_match| ListItem::new(found_match_to_span(found_match)))
                .collect::<Vec<_>>(),
        )
        .block(
            Block::default()
                .title("Search Results")
                .borders(Borders::ALL),
        );
        f.render_widget(search_results, search_results_l);

        let replace_preview_b = Block::default()
            .title("Replace Preview")
            .borders(Borders::ALL);
        f.render_widget(replace_preview_b, replace_review_l);
    }

    {
        let e = app.events.lock().unwrap();
        let events = List::new(
            e.iter()
                .map(|event| ListItem::new(Span::raw(event)))
                .collect::<Vec<_>>(),
        )
        .block(Block::default().title("Events").borders(Borders::ALL));
        f.render_widget(events, layout[2]);
    }
}

fn found_match_to_span(found_match: &FoundMatch) -> Text<'_> {
    let mut text = Text::default();

    text.lines
        .push(Spans::from(vec![Span::raw(&found_match.file_path)]));

    let mut prev_line = None;

    for (line_num, (start, end), line) in found_match.context.iter() {
        let line_num = *line_num;
        let start = *start as usize;
        let end = *end as usize;

        if let Some(prev) = prev_line {
            if prev + 1 != line_num {
                text.lines.push(Spans(vec![Span::raw("-----")]));
            }
        }
        prev_line = Some(line_num);

        text.lines.push(Spans::from(vec![
            Span::styled(
                format!("{:>4}: ", line_num),
                Style::default().fg(Color::Gray),
            ),
            Span::raw(&line[0..start]),
            Span::styled(&line[start..end], Style::default().fg(Color::Yellow)),
            Span::raw(&line[end..]),
        ]));
    }

    text
}
