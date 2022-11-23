use std::{borrow::BorrowMut, error::Error};

use tui::{interactive_form::InteractiveForm, widgets::TextInputState};

use crate::{event_log::EventLog, found_match::FoundMatch, fqcn::Fqcn, rg_worker_thread::RgWorker};

#[tui::macros::interactive_form]
pub struct Inputs {
    #[default("event")]
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
    SearchingFqcn,
    SearchingIdent,
}

pub struct App {
    pub base_dir: String,
    pub inputs: Inputs,
    pub events: EventLog,
    pub search_state: SearchState,

    pub results_scroll_offset: usize,
    pub found_matches: Vec<FoundMatch>,

    workers: Vec<RgWorker>,
}

impl App {
    pub fn new(base_dir: String) -> App {
        let mut ret = App {
            base_dir,
            search_state: SearchState::Idle,
            events: Default::default(),
            inputs: Default::default(),
            results_scroll_offset: 0,
            found_matches: vec![],
            workers: vec![],
        };
        ret.inputs.focus_input(0);
        ret.inputs.search_button.read_only(true);
        ret.inputs.replace_button.read_only(true);
        ret
    }

    pub fn check_search_done(&mut self) {
        if self.search_state == SearchState::SearchingIdent {
            for worker in self.workers.iter() {
                let mut results = worker.results();
                self.events
                    .info(format!("app: got {} matches from worker", results.len()));
                self.found_matches.append(results.borrow_mut());
            }
        }

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
                // try parsing fqcn
                if let Some(fqcn) = Fqcn::new(self.inputs.search_for_ident.get_value()) {
                    self.set_searching_and_clear_results(SearchState::SearchingFqcn);
                    self.search_for_fqcn(fqcn);
                } else {
                    self.set_searching_and_clear_results(SearchState::SearchingIdent);
                    self.search_for_raw_ident(self.inputs.search_for_ident.get_value().to_owned());
                }
            }

            SearchState::SearchingFqcn | SearchState::SearchingIdent => {
                if let Err(e) = self.kill_workers() {
                    self.log_error("error stopping search")(e);
                }

                self.set_idle();
            }
        }
    }

    fn search_for_fqcn(&mut self, fqcn: Fqcn) {
        // find all files that reference the entire FQCN
        let fqcn_worker = RgWorker::new(
            "fqcn_worker",
            self.events.clone(),
            &[
                "--json",
                "-C1",
                &format!("\\b{}\\b", fqcn.value()),
                &self.base_dir,
            ],
        );

        if let Err(err) = fqcn_worker {
            self.log_error("Error starting `rg` (fqcn)")(err);
            return;
        }

        let worker = fqcn_worker.unwrap();
        let pid = worker.pid();
        self.workers.push(worker);
        self.events.info(format!("start `rg` (fqcn): {}", pid));
    }

    fn search_for_raw_ident(&mut self, ident: String) {
        let rg_worker = RgWorker::new(
            "ident",
            self.events.clone(),
            &["--json", "-C1", &format!("\\b{}\\b", ident), &self.base_dir],
        );

        if let Err(err) = rg_worker {
            self.log_error("Error starting `rg`")(err);
            return;
        }

        let worker = rg_worker.unwrap();

        let pid = worker.pid();
        self.workers.push(worker);
        self.events.info(format!("start `rg` ident: {}", pid));
    }

    fn log_error(&self, message: &str) -> impl FnMut(Box<dyn Error>) -> Box<dyn Error> {
        let mut events = self.events.clone();
        let msg = message.to_owned();

        move |err| {
            events.error(format!("app: {}: {}", msg, err));
            err
        }
    }

    fn kill_workers(&mut self) -> Result<(), Box<dyn Error>> {
        if self.workers.is_empty() {
            return Ok(());
        }

        let workers = std::mem::take(&mut self.workers);
        for mut worker in workers {
            worker
                .kill_and_wait()
                .map_err(self.log_error("error killing worker"))?;
        }
        self.events.info("cleared workers".to_string());
        Ok(())
    }

    fn set_idle(&mut self) {
        self.inputs.search_button.set_value("Search");
        self.search_state = SearchState::Idle;
    }

    fn set_searching_and_clear_results(&mut self, state: SearchState) {
        self.inputs.search_button.set_value("Searching...");
        self.search_state = state;
        self.found_matches.clear();
    }
}
