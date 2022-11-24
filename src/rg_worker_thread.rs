use std::io::Read;
use std::mem;
use std::ops::Range;
use std::sync::Arc;
use std::{
    error::Error,
    process::{Child, ChildStdout, Command, Stdio},
    thread::{self, JoinHandle},
};

use parking_lot::{Mutex, MutexGuard};
use serde_json::Value;

use crate::event_log::EventLog;
use crate::matched_file::{Line, Match, MatchType, MatchedFile};

pub struct RgWorker {
    name: String,
    pid: u32,
    process: Child,
    thread: Option<JoinHandle<()>>,
    results: Arc<Mutex<Vec<MatchedFile>>>,
}

impl RgWorker {
    pub fn new<S>(name: S, events: EventLog, args: &[&str]) -> Result<RgWorker, Box<dyn Error>>
    where
        S: Into<String>,
    {
        let name = name.into();
        let mut process = Command::new("rg")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let results: Arc<Mutex<Vec<MatchedFile>>> = Default::default();
        let pid = process.id();
        let child_stdout = process.stdout.take().unwrap();
        let thread = thread::spawn(Self::worker_impl_factory(
            name.clone(),
            events,
            results.clone(),
            child_stdout,
        ));

        Ok(RgWorker {
            name,
            pid,
            process,
            thread: Some(thread),
            results,
        })
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn kill_and_wait(&mut self) -> Result<(), Box<dyn Error>> {
        if self.process.try_wait().is_err() {
            self.process.kill()?;
        }

        self.thread
            .take()
            .map(|thread| {
                thread
                    .join()
                    .map_err(|err| format!("{} error: {:?}", self.name, err).into())
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

    pub fn results(&self) -> MutexGuard<Vec<MatchedFile>> {
        self.results.lock()
    }

    fn worker_impl_factory(
        name: String,
        mut events: EventLog,
        matches: Arc<Mutex<Vec<MatchedFile>>>,
        mut child_stdout: ChildStdout,
    ) -> impl FnOnce() {
        move || {
            let mut buf = vec![0u8; 4096];
            let mut str_buf = String::new();
            let mut finished = false;
            let mut in_progress_found = MatchedFile::default();

            events.info(format!("rg {}: waiting for stdout", name));

            loop {
                let num_read = child_stdout.read(&mut buf).unwrap();
                if num_read == 0 {
                    events.info(format!("rg {}: end of file", name));
                    finished = true;
                }

                let as_str = std::str::from_utf8(&buf[0..num_read]).unwrap();

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
                        Self::handle_command(
                            &name,
                            &mut in_progress_found,
                            &mut events,
                            &matches,
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
        name: &str,
        in_progress: &mut MatchedFile,
        events: &mut EventLog,
        matches: &Arc<Mutex<Vec<MatchedFile>>>,
        command: Value,
    ) {
        if command["type"] == "begin" {
            in_progress.file_path = command["data"]["path"]["text"].as_str().unwrap().to_owned();
        }

        if command["type"] == "end" {
            let mut found = MatchedFile::default();
            mem::swap(in_progress, &mut found);
            events.info(format!("rg {}: match in `{:?}`", name, found.file_path));
            matches.lock().push(found);
        }

        if command["type"] == "context" {
            Self::push_context(in_progress, &command, None);
        }

        if command["type"] == "match" {
            let subs = &command["data"]["submatches"][0];
            let start = subs["start"].as_u64().unwrap() as usize;
            let end = subs["end"].as_u64().unwrap() as usize;
            Self::push_context(in_progress, &command, Some(start..end));
        }
    }

    fn push_context(
        found_match: &mut MatchedFile,
        command: &Value,
        match_range: Option<Range<usize>>,
    ) {
        let line_num = command["data"]["line_number"].as_u64().unwrap() as usize;
        let value = command["data"]["lines"]["text"]
            .as_str()
            .unwrap()
            .to_owned();

        let line = if let Some(match_range) = match_range {
            Line::Match(Match {
                match_type: MatchType::Arbitrary,
                line_num,
                match_range,
                value,
            })
        } else {
            Line::Context { line_num, value }
        };

        found_match.lines.push(line)
    }
}
