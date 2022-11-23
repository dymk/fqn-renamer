use crossterm::event::Event;
use tui::{widgets::TextInputState};

#[tui::macros::interactive_form]
pub struct Inputs {
    pub search_for_ident: TextInputState,
    #[default("Preview")]
    pub preview_button: TextInputState,
    pub replace_with_ident: TextInputState,
    #[default("Replace")]
    pub replace_button: TextInputState
}

#[derive(Default)]
pub struct App {
    base_dir: String,
    pub inputs: Inputs,
    pub events: Vec<Event>,
}

impl App {
    pub fn new(base_dir: String) -> App {
        let mut ret = App {
            base_dir,
            ..Default::default()
        };
        ret.inputs.preview_button.read_only(true);
        ret.inputs.replace_button.read_only(true);
        ret
    }
}