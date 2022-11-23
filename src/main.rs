mod app;

use app::App;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{env, error::Error, io};
use tui::{
    backend::{Backend, CrosstermBackend},
    interactive_form::InteractiveForm,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, List, ListItem, TextInput},
    Frame, Terminal,
};

fn main() -> Result<(), Box<dyn Error>> {
    // main argument parsing
    let base_dir = env::args().nth(0).expect("arg0: base dir to search");

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

        let event = event::read()?;
        app.events.push(event);

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

        f.render_interactive(search_input, l[0], &mut app.inputs.search_for_ident);

        let preview_button = TextInput::new()
            .disable_cursor(true)
            .alignment(tui::layout::Alignment::Center)
            .focused_style(focused_style())
            .block(default_block().borders(Borders::ALL));
        f.render_interactive(preview_button, l[1], &mut app.inputs.preview_button)
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

        f.render_interactive(search_input, l[0], &mut app.inputs.replace_with_ident);

        let replace_button = TextInput::new()
            .focused_style(focused_style())
            .disable_cursor(true)
            .alignment(tui::layout::Alignment::Center)
            .block(default_block().borders(Borders::ALL));
        f.render_interactive(replace_button, l[1], &mut app.inputs.replace_button)
    }

    // Results / Replacement Preview area
    {
        let l = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(layout[1]);
        let search_results_l = l[0];
        let replace_review_l = l[1];

        let search_results_b = Block::default().title("Search Results").borders(Borders::ALL);
        f.render_widget(search_results_b, search_results_l);

        let replace_preview_b = Block::default().title("Replace Preview").borders(Borders::ALL);
        f.render_widget(replace_preview_b, replace_review_l);
    }

    {}

    // let table = Table::new(
    //     app.input_states
    //         .iter()
    //         .enumerate()
    //         .map(|(idx, input_state)| {
    //             Row::new(vec![
    //                 Cell::from(Span::raw(format!("Input {}", idx + 1))),
    //                 Cell::from(Span::styled(
    //                     input_state.get_value(),
    //                     Style::default().add_modifier(Modifier::BOLD),
    //                 )),
    //             ])
    //         })
    //         .collect::<Vec<_>>(),
    // )
    // .widths(&[Constraint::Min(10), Constraint::Percentage(100)])
    // .block(Block::default().title("Input Values").borders(Borders::ALL));
    // f.render_widget(table, layout[2]);

    let events = List::new(
        app.events
            .iter()
            .rev()
            .map(|event| ListItem::new(Span::raw(format!("{:?}", event))))
            .collect::<Vec<_>>(),
    )
    .block(Block::default().title("Events").borders(Borders::ALL));
    f.render_widget(events, layout[2]);
}
