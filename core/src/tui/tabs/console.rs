use crate::parser::entry::{LogEntry, LogLevel};
use crate::parser::filters::LogFilter;
use ratatui::layout::Alignment;
use ratatui::widgets::block::Title;
use ratatui::widgets::BorderType;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

pub struct ConsoleTab<'a> {
    log_entries: &'a [LogEntry],
    filter: Option<&'a Box<dyn LogFilter>>,
    search_pattern: Option<&'a str>,
}

impl<'a> ConsoleTab<'a> {
    pub fn new(
        log_entries: &'a [LogEntry],
        filter: Option<&'a Box<dyn LogFilter>>,
        search_pattern: Option<&'a str>,
    ) -> Self {
        Self {
            log_entries,
            filter,
            search_pattern,
        }
    }

    fn get_filtered_entries(&self) -> Vec<&LogEntry> {
        match self.filter {
            Some(filter) => self.log_entries.iter().filter(|e| filter.matches(e)).collect(),
            None => self.log_entries.iter().collect(),
        }
    }

    fn log_level_color(level: LogLevel) -> Color {
        match level {
            LogLevel::Debug => Color::DarkGray,
            LogLevel::Info => Color::White,
            LogLevel::Warning => Color::Yellow,
            LogLevel::Error => Color::Red,
            LogLevel::Fatal => Color::Magenta,
        }
    }

    fn create_list_item(&self, entry: &'a LogEntry, index: usize, line_number_width: usize) -> ListItem<'a> {
        let color = Self::log_level_color(entry.level);
        let line_number = index + 1;
        let timestamp = entry.timestamp.format("%H:%M:%S");

        let mut content = vec![
            Span::styled(
                format!(":{:>width$} ", line_number, width = line_number_width),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!("[{}] ", timestamp),
                Style::default().fg(Color::DarkGray),
            ),
        ];

        if let Some(location) = entry.location_string() {
            content.push(Span::styled(&entry.message, Style::default().fg(color)));
            content.push(Span::raw(" "));
            content.push(Span::styled(location, Style::default().fg(Color::Cyan)));
        } else {
            content.push(Span::styled(&entry.raw_line, Style::default().fg(color)));
        }

        let line = match self.search_pattern {
            Some(pattern) if entry.message.contains(pattern) || entry.raw_line.contains(pattern) => {
                Line::from(content).patch_style(Style::default().add_modifier(Modifier::REVERSED))
            }
            _ => Line::from(content),
        };

        ListItem::new(line)
    }

    fn build_title(&self) -> String {
        let keybindings = "[j/k: Line | Ctrl+U/D: Half | Ctrl+F/B: Page | gg/G: Top/Bot | zz/zt/zb: View | n/N: Search]";
        match self.filter {
            Some(filter) => format!(" Console ({}) {} ", filter.description(), keybindings),
            None => format!(" Console {} ", keybindings),
        }
    }

    fn create_block(&self) -> Block<'_> {
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .title(Title::from(self.build_title()).alignment(Alignment::Center))
    }

    fn render_empty(&self, frame: &mut Frame, area: Rect, state: &mut ListState) {
        let list = List::new(Vec::<ListItem>::new()).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title(Title::from(" Console (empty) ").alignment(Alignment::Center)),
        );
        frame.render_stateful_widget(list, area, state);
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, state: &mut ListState) {
        let filtered_entries = self.get_filtered_entries();

        if filtered_entries.is_empty() {
            self.render_empty(frame, area, state);
            return;
        }

        let line_number_width = filtered_entries.len().to_string().len().max(3);

        let items: Vec<ListItem> = filtered_entries
            .iter()
            .enumerate()
            .map(|(index, entry)| self.create_list_item(entry, index, line_number_width))
            .collect();

        let list = List::new(items)
            .block(self.create_block())
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol(">> ");

        frame.render_stateful_widget(list, area, state);
    }
}
