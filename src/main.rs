mod app;
mod controller;
mod event_log;
mod fqcn;
mod fqcn_processor;
mod matched_file;
mod rg_worker;
mod scrollable;
mod ui;

use app::App;
use controller::AppEvent;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    env,
    error::Error,
    io,
    sync::mpsc::{channel, Receiver},
    thread,
};
use tui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};

fn main() -> Result<(), Box<dyn Error>> {
    // main argument parsing
    let base_dir = env::args().nth(1).unwrap_or_else(|| ".".to_owned());

    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // create app and run it
    let (events_tx, events_rx) = channel();

    // queue up the first redraw of the app
    events_tx.send(AppEvent::Redraw)?;

    let mut app = App::new(base_dir, events_tx.clone());
    app.search_input_submitted();

    // start polling for user input events
    thread::spawn(move || loop {
        match event::read() {
            Ok(event) => events_tx.send(AppEvent::Crossterm(event)),
            Err(e) => events_tx.send(AppEvent::Abort(e.to_string())),
        }
        .unwrap();
    });

    let res = run_app(&mut app, &mut terminal, events_rx);

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

fn run_app<B: Backend>(
    app: &mut App,
    terminal: &mut Terminal<B>,
    events_rx: Receiver<AppEvent>,
) -> Result<(), Box<dyn Error>> {
    loop {
        let event = events_rx.recv()?;
        if !controller::handle_event(event, app, terminal)? {
            return Ok(());
        }
    }
}
