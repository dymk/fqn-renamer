use parking_lot::{MappedMutexGuard, Mutex, MutexGuard};
use std::{borrow::BorrowMut, collections::VecDeque, sync::Arc};

#[derive(Default, Clone)]
pub struct EventLog {
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    last: usize,
    events: VecDeque<LogLine>,
}

pub struct LogLine {
    pub num: usize,
    pub level: Level,
    pub value: String,
}

pub enum Level {
    Info,
    Error,
}

impl EventLog {
    pub fn info(&mut self, str: String) {
        self.push(Level::Info, str);
    }
    pub fn error(&mut self, str: String) {
        self.push(Level::Error, str);
    }

    fn push(&mut self, level: Level, str: String) {
        let mut guard = self.state.lock();
        let state = guard.borrow_mut();
        let last = state.last;
        state.events.push_front(LogLine {
            num: last,
            level,
            value: str,
        });
        state.last += 1;
        if state.events.len() > 20 {
            state.events.pop_back();
        }
    }

    pub fn list(&self) -> MappedMutexGuard<VecDeque<LogLine>> {
        MutexGuard::map(self.state.lock(), |state| &mut state.events)
    }
}
