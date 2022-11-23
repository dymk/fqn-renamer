use std::collections::VecDeque;

#[derive(Default)]
pub struct EventLog {
    events: VecDeque<String>,
}
impl EventLog {
    pub fn push(&mut self, str: String) {
        self.events.push_front(str);
        if self.events.len() > 10 {
            self.events.pop_back();
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &String> + '_ {
        self.events.iter().rev()
    }
}
