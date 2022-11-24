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
use crate::matched_file::{Line, MatchedFile};

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
            let mut in_progress_found = MatchedFileBuilder::default();

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
        builder: &mut MatchedFileBuilder,
        events: &mut EventLog,
        matches: &Arc<Mutex<Vec<MatchedFile>>>,
        command: Value,
    ) {
        // events.info(format!("rg command: {}", command));

        if command["type"] == "begin" {
            builder.file_path = command["data"]["path"]["text"].as_str().unwrap().to_owned();
        }

        if command["type"] == "end" {
            let found = builder.build();
            events.info(format!("rg {}: match in `{:?}`", name, found.file_path()));
            matches.lock().push(found);
        }

        if command["type"] == "context" {
            Self::push_context(builder, &command, vec![]);
        }

        if command["type"] == "match" {
            let subs = command["data"]["submatches"]
                .as_array()
                .unwrap()
                .iter()
                .map(|submatch| {
                    let start = submatch["start"].as_u64().unwrap() as usize;
                    let end = submatch["end"].as_u64().unwrap() as usize;
                    start..end
                })
                .collect();
            Self::push_context(builder, &command, subs);
        }
    }

    fn push_context(
        builder: &mut MatchedFileBuilder,
        command: &Value,
        submatches: Vec<Range<usize>>,
    ) {
        // lines are 1-indexed from rg, sub 1 to make it zero indexed
        let line_num = command["data"]["line_number"].as_u64().unwrap() as usize - 1;
        let value = command["data"]["lines"]["text"]
            .as_str()
            .unwrap()
            .to_owned();

        builder.lines.push(Line::new(line_num, value, submatches));
    }
}

#[derive(Default)]
struct MatchedFileBuilder {
    file_path: String,
    lines: Vec<Line>,
}
impl MatchedFileBuilder {
    fn build(&mut self) -> MatchedFile {
        MatchedFile::new(mem::take(&mut self.file_path), mem::take(&mut self.lines))
    }
}
