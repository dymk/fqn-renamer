#[derive(Default)]
pub struct Scrollable<T> {
    offset: usize,
    max_len: usize,
    vec: Vec<T>,
}

impl<T> Scrollable<T> {
    pub fn new(offset: usize, max_len: usize) -> Self {
        Self {
            offset,
            max_len,
            vec: vec![],
        }
    }
    pub fn push(&mut self, t: impl FnOnce() -> T) {
        if self.offset != 0 {
            self.offset -= 1;
            return;
        }
        if self.max_len == 0 {
            return;
        }

        self.max_len -= 1;
        self.vec.push(t());
    }
    pub fn get(self) -> Vec<T> {
        self.vec
    }
}
