use crate::{app::App, event_log};

use crate::fqcn::Fqcn;
use crate::matched_file::MatchedFile;
use crate::scrollable::Scrollable;

use std::cell::RefCell;
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, TextInput},
    Frame,
};

pub fn draw<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let layout = Layout::default()
        .horizontal_margin(10)
        .vertical_margin(2)
        .constraints(
            [
                // inputs
                Constraint::Length(9),
                // results
                Constraint::Min(10),
                // event log
                Constraint::Length(if app.show_events { 30 } else { 2 }),
            ]
            .as_ref(),
        )
        .split(f.size());

    let inputs_layout = Layout::default()
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Length(3),
            ]
            .as_ref(),
        )
        .split(layout[0]);

    let default_block = || {
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray))
    };
    let focused_style = || {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    };

    // Base dir input
    {
        let base_dir = Paragraph::new(Text::from(app.base_dir.as_str())).block(
            default_block()
                .title("Search Directory")
                .borders(Borders::ALL),
        );
        f.render_widget(base_dir, inputs_layout[0]);
    }

    // "Search" input and preview button
    {
        let l = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(10), Constraint::Length(20)].as_ref())
            .split(inputs_layout[1]);

        let search_input = TextInput::new()
            .block(default_block().title("Search").borders(Borders::ALL))
            .focused_style(focused_style())
            .styler(make_fqcn_styler())
            .placeholder_text("Identifier or FQCN");

        f.render_interactive(search_input, l[0], &app.inputs.search_for_ident);

        let preview_button = TextInput::new()
            .disable_cursor(true)
            .alignment(tui::layout::Alignment::Center)
            .focused_style(focused_style())
            .block(default_block().borders(Borders::ALL));
        f.render_interactive(preview_button, l[1], &app.inputs.search_button)
    }

    // "Replace" input and preview button
    {
        let l = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(10), Constraint::Length(20)].as_ref())
            .split(inputs_layout[2]);

        let search_input = TextInput::new()
            .focused_style(focused_style())
            .block(default_block().title("Replace").borders(Borders::ALL))
            .styler(make_fqcn_styler())
            .placeholder_text("Identifier or FQCN");

        f.render_interactive(search_input, l[0], &app.inputs.replace_with_ident);

        let replace_button = TextInput::new()
            .focused_style(focused_style())
            .disable_cursor(true)
            .alignment(tui::layout::Alignment::Center)
            .block(default_block().borders(Borders::ALL));
        f.render_interactive(replace_button, l[1], &app.inputs.replace_button)
    }

    // Results / Replacement Preview area
    {
        let l = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .split(layout[1]);
        let search_results_l = l[0];
        let replace_review_l = l[1];

        let matches = &app.found_matches;
        let num_files = matches.len();
        let num_matches: usize = matches.iter().map(|fm| fm.lines().count()).sum();

        let mut search_scrollable = RefCell::new(Scrollable::new(
            app.results_scroll_offset,
            search_results_l.height as usize,
        ));

        let mut first = true;
        for found_match in matches.iter() {
            if !first {
                search_scrollable.borrow_mut().push(|| Spans::from(vec![]));
            }
            first = false;
            add_match_to_scrollable(&mut search_scrollable, found_match, true);
        }

        let search_results = Paragraph::new(Text::from(search_scrollable.take().get())).block(
            Block::default()
                .title(Spans::from(vec![
                    Span::raw("Search Results "),
                    Span::raw(format!("({} files, {} matches)", num_files, num_matches)),
                ]))
                .borders(Borders::ALL),
        );
        f.render_widget(search_results, search_results_l);

        let replacements = &app.replacments;
        let mut preview_scrollable = RefCell::new(Scrollable::new(
            app.results_scroll_offset,
            search_results_l.height as usize,
        ));

        let mut first = true;
        for found_match in replacements.iter() {
            if !first {
                preview_scrollable.borrow_mut().push(|| Spans::from(vec![]));
            }
            first = false;
            add_match_to_scrollable(&mut preview_scrollable, found_match, false);
        }

        let replace_preview_b = Paragraph::new(Text::from(preview_scrollable.take().get())).block(
            Block::default()
                .title(Span::raw("Replace Preview"))
                .borders(Borders::ALL),
        );

        f.render_widget(replace_preview_b, replace_review_l);
    }

    let event_block = Block::default()
        .title("Event Log (toggle: ctrl+l)")
        .borders(Borders::ALL);
    if app.show_events {
        let events_list = app.events.list();
        let events = List::new(
            events_list
                .iter()
                .map(|line| {
                    let num = format!("{:>4}", line.num);
                    let num = match line.level {
                        event_log::Level::Info => Span::raw(num),
                        event_log::Level::Error => {
                            Span::styled(num, Style::default().fg(Color::Red))
                        }
                    };

                    ListItem::new(Spans::from(vec![
                        num,
                        Span::raw(" "),
                        Span::raw(&line.value),
                    ]))
                })
                .collect::<Vec<_>>(),
        )
        .block(event_block);
        f.render_widget(events, layout[2]);
    } else {
        f.render_widget(event_block, layout[2]);
    }
}

fn add_match_to_scrollable<'a>(
    scrollable: &mut RefCell<Scrollable<Spans<'a>>>,
    found_match: &'a MatchedFile,
    is_preview: bool,
) {
    let section_sep = format!("    |{}", "-".repeat(10));
    let match_color = if is_preview {
        Color::Yellow
    } else {
        Color::Rgb(181, 96, 43)
    };

    scrollable.borrow_mut().push(|| {
        let mut v = vec![Span::styled(
            found_match.file_path(),
            Style::default().fg(tui::style::Color::Magenta),
        )];

        if is_preview {
            v.push(Span::raw(" "));
            v.push(Span::styled(
                format!("({})", found_match.lines().count()),
                Style::default().fg(Color::Blue),
            ));
        }

        v.into()
    });

    let mut prev_line = None;

    for line in found_match.lines() {
        let line_num = line.num();

        if let Some(prev) = prev_line {
            if prev + 1 != line_num {
                scrollable
                    .borrow_mut()
                    .push(|| Spans::from(vec![Span::raw(section_sep.clone())]));
            }
        }
        prev_line = Some(line_num);

        scrollable.borrow_mut().push(|| {
            let line_num_prefix = std::iter::once(Span::styled(
                // add one to make line numbers one-indexed
                format!("{:>4}| ", line_num + 1),
                Style::default().fg(Color::DarkGray),
            ));

            let highlighted = line.iter().map(|(is_match, substr)| {
                if is_match {
                    Span::styled(substr, Style::default().fg(match_color))
                } else {
                    Span::raw(substr)
                }
            });

            line_num_prefix
                .chain(highlighted)
                .collect::<Vec<_>>()
                .into()
        });
    }
}

fn make_fqcn_styler() -> impl FnOnce(bool, &str) -> Spans {
    |_focused, contents| {
        if let Some(fqcn) = Fqcn::new(contents) {
            vec![
                Span::styled(
                    fqcn.package_with_trailing().to_owned(),
                    Style::default().fg(Color::Green),
                ),
                Span::styled(fqcn.ident().to_owned(), Style::default().fg(Color::Blue)),
            ]
            .into()
        } else {
            Span::raw(contents).into()
        }
    }
}
