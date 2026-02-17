use crate::history::{BuildHistoryEntry, ExecutionHistoryEntry};
use ratatui::{
    layout::{Alignment, Constraint, Rect},
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, Cell, Row, Table},
    Frame,
};

pub struct HistoryTab<'a> {
    history: &'a [BuildHistoryEntry],
}

impl<'a> HistoryTab<'a> {
    pub fn new(history: &'a [BuildHistoryEntry]) -> Self {
        Self { history }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let header_cells = [
            "Timestamp",
            "Preset",
            "Duration",
            "Status",
            "Errors",
            "Warnings",
        ]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
        let header = Row::new(header_cells).height(1).bottom_margin(1);

        let rows = self.history.iter().rev().take(20).map(|entry| {
            let status_color = if entry.success {
                Color::Green
            } else {
                Color::Red
            };
            let status_text = if entry.success { "✓ OK" } else { "✗ FAIL" };

            Row::new(vec![
                Cell::from(entry.timestamp.format("%Y-%m-%d %H:%M").to_string()),
                Cell::from(entry.preset.clone()),
                Cell::from(format!("{:.1}s", entry.duration)),
                Cell::from(status_text).style(Style::default().fg(status_color)),
                Cell::from(entry.error_count.to_string()),
                Cell::from(entry.warning_count.to_string()),
            ])
        });

        let widths = [
            Constraint::Length(16),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
        ];

        let title = format!(" Build History ({}) ", self.history.len());

        let table = Table::new(rows, widths).header(header).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title(ratatui::widgets::block::Title::from(title).alignment(Alignment::Center)),
        );

        frame.render_widget(table, area);
    }

    pub fn render_exec_history(exec_history: &[ExecutionHistoryEntry], frame: &mut Frame, area: Rect) {
        let header_cells = [
            "Timestamp",
            "Executable",
            "Duration",
            "Status",
            "Failure Reason",
            "Errors",
            "Warnings",
            "Metrics",
        ]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
        let header = Row::new(header_cells).height(1).bottom_margin(1);

        let rows = exec_history.iter().rev().take(20).map(|entry| {
            let status_color = if entry.success {
                Color::Green
            } else {
                Color::Red
            };
            let status_text = if entry.success { "✓ OK" } else { "✗ FAIL" };
            let failure_text = entry.failure_reason.as_deref().unwrap_or("-");

            Row::new(vec![
                Cell::from(entry.timestamp.format("%Y-%m-%d %H:%M").to_string()),
                Cell::from(entry.executable_name.clone()),
                Cell::from(format!("{:.1}s", entry.duration)),
                Cell::from(status_text).style(Style::default().fg(status_color)),
                Cell::from(failure_text).style(Style::default().fg(Color::Red)),
                Cell::from(entry.error_count.to_string()),
                Cell::from(entry.warning_count.to_string()),
                Cell::from(entry.metric_count.to_string()),
            ])
        });

        let widths = [
            Constraint::Length(16),
            Constraint::Length(15),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(20),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
        ];

        let title = format!(" Execution History ({}) ", exec_history.len());

        let table = Table::new(rows, widths).header(header).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Magenta))
                .title(ratatui::widgets::block::Title::from(title).alignment(Alignment::Center)),
        );

        frame.render_widget(table, area);
    }
}
