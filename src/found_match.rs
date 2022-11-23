use std::ops::Range;

#[derive(Debug, Default)]
pub struct FoundMatch {
    pub file_path: String,
    pub line_number: u64,
    pub start: u64,
    pub end: u64,
    pub context: Vec<(u64, Range<u64>, String)>,
}

impl FoundMatch {
    pub fn matching_lines(&self) -> impl Iterator<Item = &String> {
        self.context
            .iter()
            .filter(|line| !line.1.is_empty())
            .map(|line| &line.2)
    }
}
