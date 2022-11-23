#[derive(Debug, Default)]
pub struct FoundMatch {
    pub file_path: String,
    pub line_number: u64,
    pub start: u64,
    pub end: u64,
    pub context: Vec<(u64, (u64, u64), String)>,
}
