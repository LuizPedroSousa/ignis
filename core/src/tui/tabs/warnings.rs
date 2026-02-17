use crate::parser::entry::{LogEntry, LogLevel};
use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{block::Title, Block, BorderType, Borders, List, ListItem, ListState},
    Frame,
};
use std::path::PathBuf;

pub struct WarningsTab<'a> {
    log_entries: &'a [LogEntry],
}

#[derive(Debug, Clone)]
pub struct WarningLocation {
    pub file_path: PathBuf,
    pub line: usize,
    pub column: Option<usize>,
}

impl<'a> WarningsTab<'a> {
    pub fn new(log_entries: &'a [LogEntry]) -> Self {
        Self { log_entries }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, state: &mut ListState) {
        let warnings: Vec<&LogEntry> = self
            .log_entries
            .iter()
            .filter(|e| e.level == LogLevel::Warning)
            .collect();

        let items: Vec<ListItem> = warnings
            .iter()
            .map(|entry| {
                let content = if let Some(location) = entry.location_string() {
                    vec![
                        Span::styled(&entry.message, Style::default().fg(Color::Yellow)),
                        Span::raw(" "),
                        Span::styled(location, Style::default().fg(Color::Cyan)),
                    ]
                } else {
                    vec![Span::styled(
                        &entry.raw_line,
                        Style::default().fg(Color::Yellow),
                    )]
                };

                ListItem::new(Line::from(content))
            })
            .collect();

        let title = format!(" Warnings ({}) [↑↓/jk: Scroll | Enter: Open File] ", warnings.len());
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(Title::from(title).alignment(Alignment::Center)),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol(">> ");

        frame.render_stateful_widget(list, area, state);
    }

    pub fn get_selected_location(&self, selected: usize) -> Option<WarningLocation> {
        let warnings: Vec<&LogEntry> = self
            .log_entries
            .iter()
            .filter(|e| e.level == LogLevel::Warning)
            .collect();

        let entry = warnings.get(selected)?;
        self.parse_warning_location(entry)
    }

    fn parse_warning_location(&self, entry: &LogEntry) -> Option<WarningLocation> {
        let text = if entry.message.is_empty() {
            &entry.raw_line
        } else {
            &entry.raw_line
        };

        let parts: Vec<&str> = text.split_whitespace().collect();

        for part in parts.iter().rev() {
            if part.contains(':') && (part.starts_with('/') || part.contains("src/") || part.contains("examples/")) {
                let location_parts: Vec<&str> = part.split(':').collect();

                if location_parts.len() >= 2 {
                    let file_path = PathBuf::from(location_parts[0]);

                    if let Ok(line) = location_parts[1].parse::<usize>() {
                        let column = if location_parts.len() >= 3 {
                            location_parts[2].parse::<usize>().ok()
                        } else {
                            None
                        };

                        return Some(WarningLocation {
                            file_path,
                            line,
                            column,
                        });
                    }
                }
            }
        }

        None
    }
}
