use std::collections::VecDeque;

#[derive(Default)]
pub struct EventLog {
    last: usize,
    events: VecDeque<(usize, String)>,
}
impl EventLog {
    pub fn push(&mut self, str: String) {
        self.events.push_front((self.last, str));
        self.last += 1;
        if self.events.len() > 20 {
            self.events.pop_back();
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &(usize, String)> + '_ {
        self.events.iter()
    }
}
