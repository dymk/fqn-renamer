use std::{
    error::Error,
    io::Read,
    mem,
    process::{Child, ChildStdout, Command, Stdio},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
};

use serde_json::Value;
use tui::{interactive_form::InteractiveForm, widgets::TextInputState};

use crate::event_log::EventLog;

#[tui::macros::interactive_form]
pub struct Inputs {
    #[default("com.example.foo.Foo")]
    pub search_for_ident: TextInputState,
    #[default("Search")]
    pub search_button: TextInputState,
    pub replace_with_ident: TextInputState,
    #[default("Replace")]
    pub replace_button: TextInputState,
}

#[derive(Eq, PartialEq, Debug)]
pub enum SearchState {
    Idle,
    Searching,
}

struct RgThread {
    command: Vec<String>,
    pid: u32,
    process: Child,
    thread: Option<JoinHandle<()>>,
}

impl RgThread {
    pub fn new<F, C>(
        command: &str,
        args: &[&str],
        worker_factory: F,
    ) -> Result<RgThread, Box<dyn Error>>
    where
        F: FnOnce(ChildStdout) -> C,
        C: FnOnce() + Send + 'static,
    {
        let mut process = Command::new(command)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let pid = process.id();
        let child_stdout = process.stdout.take().unwrap();
        let thread = thread::spawn(worker_factory(child_stdout));

        let mut all_commands = vec![command.to_owned()];
        args.iter()
            .for_each(|arg| all_commands.push(arg.to_string()));

        Ok(RgThread {
            command: all_commands,
            pid,
            process,
            thread: Some(thread),
        })
    }

    pub fn kill_and_wait(&mut self) -> Result<(), Box<dyn Error>> {
        if self.process.try_wait().is_err() {
            self.process.kill()?;
        }

        self.thread
            .take()
            .map(|thread| {
                thread.join().map_err(|err| {
                    format!("{} error: {:?}", self.command.first().unwrap(), err).into()
                })
            })
            .unwrap_or(Ok(()))
    }

    pub fn finished(&mut self) -> bool {
        self.thread
            .as_ref()
            .map(|thread| thread.is_finished())
            .unwrap_or(true)
            && self
                .process
                .try_wait()
                .map_or_else(|_| true, |opt| opt.is_some())
    }
}

pub struct App {
    pub base_dir: String,
    pub inputs: Inputs,
    pub events: Arc<Mutex<EventLog>>,
    pub search_state: SearchState,

    pub results_scroll_offset: usize,

    pub found_matches: Arc<Mutex<Vec<FoundMatch>>>,
    workers: Vec<RgThread>,
}

impl App {
    pub fn new(base_dir: String) -> App {
        let mut ret = App {
            base_dir,
            search_state: SearchState::Idle,
            events: Default::default(),
            inputs: Default::default(),
            results_scroll_offset: 0,
            workers: vec![],
            found_matches: Arc::new(Mutex::new(vec![])),
        };
        ret.inputs.focus_input(0);
        ret.inputs.search_button.read_only(true);
        ret.inputs.replace_button.read_only(true);
        ret
    }

    pub fn check_search_done(&mut self) {
        if self.workers.iter_mut().all(|worker| worker.finished()) {
            if let Err(e) = self.kill_workers() {
                self.log_error("Error killing workers")(e);
            }
            self.set_idle();
        }
    }

    pub fn search_input_submitted(&mut self) {
        if self.search_state == SearchState::Idle {
            self.search_button_submitted();
        }
    }

    pub fn search_button_submitted(&mut self) {
        match self.search_state {
            SearchState::Idle => {
                self.inputs.search_button.set_value("Searching...");
                self.search_state = SearchState::Searching;
                self.found_matches.lock().unwrap().clear();

                let rg_worker = RgThread::new(
                    "rg",
                    &[
                        "--json",
                        "-C1",
                        &format!("\\b{}\\b", self.inputs.search_for_ident.get_value()),
                        &self.base_dir,
                    ],
                    |child_stdout| self.make_rg_thread(child_stdout),
                );

                if let Err(err) = rg_worker {
                    self.log_error("Error starting `rg`")(err);
                    return;
                }

                let worker = rg_worker.unwrap();

                let pid = worker.pid;
                self.workers.push(worker);

                self.events
                    .lock()
                    .unwrap()
                    .push(format!("start `rg`: {}", pid));
            }

            SearchState::Searching => {
                if let Err(e) = self.kill_workers() {
                    self.log_error("error stopping search")(e);
                }

                self.set_idle();
            }
        }
    }

    fn log_error(&self, message: &str) -> impl FnMut(Box<dyn Error>) -> Box<dyn Error> {
        let events = self.events.clone();
        let msg = message.to_owned();

        move |err| {
            events.lock().unwrap().push(format!("{}: {}", msg, err));
            err
        }
    }

    fn kill_workers(&mut self) -> Result<(), Box<dyn Error>> {
        let workers = std::mem::take(&mut self.workers);
        for mut worker in workers {
            worker
                .kill_and_wait()
                .map_err(self.log_error("Error killing worker"))?;
        }
        Ok(())
    }

    fn set_idle(&mut self) {
        self.inputs.search_button.set_value("Search");
        self.search_state = SearchState::Idle;
    }

    fn make_rg_thread(&self, mut child_stdout: ChildStdout) -> impl FnOnce() {
        let shared_events = self.events.clone();
        let shared_found = self.found_matches.clone();

        move || {
            let mut buf = vec![0u8; 4096];
            let mut str_buf = String::new();
            let mut finished = false;
            let mut in_progress_found = FoundMatch::default();

            shared_events
                .lock()
                .unwrap()
                .push("Child thread waiting for rg output...".to_owned());

            loop {
                let num_read = child_stdout.read(&mut buf).unwrap();
                if num_read == 0 {
                    shared_events
                        .lock()
                        .unwrap()
                        .push("No more to read".to_owned());
                    finished = true;
                }

                let as_str = std::str::from_utf8(&buf[0..num_read]).unwrap();

                if !as_str.is_empty() {
                    shared_events
                        .lock()
                        .unwrap()
                        .push(format!("from rg: `{}`", as_str));
                }

                str_buf.push_str(as_str);

                // find location of next newline
                'no_nl: loop {
                    let cmd_end = if finished {
                        str_buf.len()
                    } else {
                        match str_buf.find('\n') {
                            Some(pos) => pos + 1,
                            None => break 'no_nl,
                        }
                    };

                    let (command, rest) = str_buf.split_at(cmd_end);
                    if !command.is_empty() {
                        let command: Value = serde_json::from_str(command).unwrap();
                        App::handle_command(
                            &mut in_progress_found,
                            &shared_events,
                            &shared_found,
                            command,
                        );
                    }

                    str_buf = rest.to_owned();
                    if finished || str_buf.is_empty() {
                        break;
                    }
                }

                if finished {
                    break;
                }
            }
        }
    }

    fn handle_command(
        in_progress_found: &mut FoundMatch,
        shared_events: &Arc<Mutex<EventLog>>,
        shared_found: &Arc<Mutex<Vec<FoundMatch>>>,
        command: Value,
    ) {
        if command["type"] == "begin" {
            in_progress_found.file_path =
                command["data"]["path"]["text"].as_str().unwrap().to_owned();
        }

        if command["type"] == "end" {
            let mut found = FoundMatch::default();
            mem::swap(in_progress_found, &mut found);

            shared_events
                .lock()
                .unwrap()
                .push(format!("push found: `{:?}`", found));

            shared_found.lock().unwrap().push(found);
        }

        if command["type"] == "context" {
            App::push_context(in_progress_found, &command, (0, 0));
        }

        if command["type"] == "match" {
            let subs = &command["data"]["submatches"][0];
            let start = subs["start"].as_u64().unwrap();
            let end = subs["end"].as_u64().unwrap();
            App::push_context(in_progress_found, &command, (start, end));
            in_progress_found.line_number = command["data"]["line_number"].as_u64().unwrap();
        }
    }

    fn push_context(found_match: &mut FoundMatch, command: &Value, highlight: (u64, u64)) {
        found_match.context.push((
            command["data"]["line_number"].as_u64().unwrap(),
            highlight,
            command["data"]["lines"]["text"]
                .as_str()
                .unwrap()
                .to_owned(),
        ));
    }
}

#[derive(Debug, Default)]
pub struct FoundMatch {
    pub file_path: String,
    pub line_number: u64,
    pub start: u64,
    pub end: u64,
    pub context: Vec<(u64, (u64, u64), String)>,
}
