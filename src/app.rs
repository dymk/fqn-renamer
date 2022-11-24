use std::{borrow::BorrowMut, error::Error, mem};

use tui::{interactive_form::InteractiveForm, widgets::TextInputState};

use crate::{
    event_log::EventLog, fqcn::Fqcn, fqcn_replacer::process_matched_file_fqcn,
    matched_file::MatchedFile, rg_worker_thread::RgWorker,
};

#[tui::macros::interactive_form]
pub struct Inputs {
    #[default("com.example.Foo")]
    pub search_for_ident: TextInputState,
    #[default("Search")]
    pub search_button: TextInputState,
    pub replace_with_ident: TextInputState,
    #[default("Replace")]
    pub replace_button: TextInputState,
}

pub enum SearchState {
    Idle,
    SearchingFqcn(Fqcn),
    SearchingIdent,
}

pub struct App {
    pub base_dir: String,
    pub inputs: Inputs,
    pub events: EventLog,
    search_state: SearchState,

    pub results_scroll_offset: usize,

    pub found_matches: Vec<MatchedFile>,
    pub replacments: Vec<MatchedFile>,

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
            replacments: vec![],
            workers: vec![],
        };
        ret.inputs.focus_input(0);
        ret.inputs.search_button.read_only(true);
        ret.inputs.replace_button.read_only(true);
        ret
    }

    pub fn check_search_done(&mut self) {
        let mut results_changed = false;

        if matches!(self.search_state, SearchState::SearchingIdent) {
            for worker in self.workers.iter() {
                let mut results = worker.results();
                self.events.info(format!(
                    "app: got {} matches from ident worker",
                    results.len()
                ));
                self.found_matches.append(results.borrow_mut());
                results_changed = true;
            }
        } else if let SearchState::SearchingFqcn(fqcn) = &self.search_state {
            for worker in self.workers.iter() {
                let results = mem::take(&mut *worker.results());
                self.events
                    .info(format!("app: got {} matches from worker", results.len()));
                let mut results = process_matched_file_fqcn(fqcn, results);
                if !results.is_empty() {
                    results_changed = true;
                }
                self.found_matches.append(&mut results);
            }
        }

        if self.workers.iter_mut().all(|worker| worker.finished()) {
            if let Err(e) = self.kill_workers() {
                self.log_error("Error killing workers")(e);
            }
            self.set_idle();
        }

        if results_changed {
            self.update_replacements();
        }
    }

    pub fn search_input_submitted(&mut self) {
        if matches!(self.search_state, SearchState::Idle) {
            self.search_button_submitted();
        }
    }

    pub fn replace_input_submitted(&mut self) {
        if !matches!(self.search_state, SearchState::Idle) {
            self.events
                .error("app: cannot do replace while searching".to_owned());
        }
    }

    pub fn update_replacements(&mut self) {
        self.replacments.clear();

        let find_ident = self.inputs.search_for_ident.get_value();
        let repl_ident = self.inputs.replace_with_ident.get_value();

        if let Some(find_fqcn) = Fqcn::new(find_ident) {
            if let Some(repl_fqcn) = Fqcn::new(repl_ident) {
                self.update_replacements_fqcn(find_fqcn, repl_fqcn);
                return;
            }
        }

        // not a valid fqcn, just do a straight identifier replacement
        let ident = if repl_ident.is_empty() {
            find_ident
        } else {
            repl_ident
        };
        for mf in self.found_matches.iter() {
            self.replacments.push(mf.replace(&|_| ident.to_owned()));
        }
    }

    fn update_replacements_fqcn(&mut self, find: Fqcn, repl: Fqcn) {
        for mf in self.found_matches.iter() {
            self.replacments.push(mf.replace(&|ident| {
                if ident == find.ident() {
                    repl.ident()
                } else if ident == find.value() {
                    repl.value()
                } else if ident == find.package() {
                    repl.package()
                } else {
                    repl.ident()
                }
                .to_owned()
            }));
        }
    }

    pub fn search_button_submitted(&mut self) {
        match self.search_state {
            SearchState::Idle => {
                // try parsing fqcn
                if let Some(fqcn) = Fqcn::new(self.inputs.search_for_ident.get_value()) {
                    self.set_searching_and_clear_results();
                    self.search_for_fqcn(fqcn);
                } else {
                    self.set_searching_and_clear_results();
                    self.search_state = SearchState::SearchingIdent;
                    self.search_for_raw_ident(self.inputs.search_for_ident.get_value().to_owned());
                }
            }

            SearchState::SearchingFqcn(_) | SearchState::SearchingIdent => {
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
                // find the thing that defines the package, references the
                // identifier (filter out the false positives later),
                // or imports the identifier (use that for filtering)
                &format!(
                    r"(^package {};?$)|(\b{}\b)|(\b{}\b)|(^import {};?$)",
                    // `package foo.Bar`
                    fqcn.package(),
                    // `Bar`
                    fqcn.ident(),
                    // `foo.Bar`
                    fqcn.value(),
                    // `import foo.Bar`
                    fqcn.value()
                ),
                &self.base_dir,
            ],
        );

        if let Err(err) = fqcn_worker {
            self.log_error("Error starting `rg` (fqcn)")(err);
            return;
        }

        self.search_state = SearchState::SearchingFqcn(fqcn);
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

        self.search_state = SearchState::SearchingIdent;
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

    fn set_searching_and_clear_results(&mut self) {
        self.events.info(format!(
            "app: starting search for `{}`",
            self.inputs.search_for_ident.get_value()
        ));
        self.inputs.search_button.set_value("Stop Search");
        self.found_matches.clear();
        self.results_scroll_offset = 0;
    }
}
