#[derive(Default)]
pub struct Scrollable<T> {
    offset: usize,
    vec: Vec<T>,
}

impl<T> Scrollable<T> {
    pub fn new(offset: usize) -> Self {
        Self {
            offset,
            vec: vec![],
        }
    }
    pub fn push(&mut self, t: impl FnOnce() -> T) {
        if self.offset > 0 {
            self.offset -= 1;
        } else {
            self.vec.push(t())
        }
    }
    pub fn get(self) -> Vec<T> {
        self.vec
    }
}
