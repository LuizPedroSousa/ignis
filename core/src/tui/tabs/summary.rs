use crate::executor::{MetricHistory, MetricType, MetricVisualization};
use crate::history::BuildStepResult;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{
        block::Title, Block, BorderType, Borders, Cell, Gauge, Paragraph, Row,
        Sparkline, Table,
    },
    Frame,
};
use std::collections::HashMap;

pub struct SummaryTab<'a> {
    steps: &'a [BuildStepResult],
}

impl<'a> SummaryTab<'a> {
    pub fn new(steps: &'a [BuildStepResult]) -> Self {
        Self { steps }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let header_cells = ["Step", "Duration", "Status", "Errors", "Warnings"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
        let header = Row::new(header_cells).height(1).bottom_margin(1);

        let rows = self.steps.iter().map(|step| {
            let status_color = if step.success {
                Color::Green
            } else {
                Color::Red
            };
            let status_text = if step.success { "✓ OK" } else { "✗ FAIL" };

            Row::new(vec![
                Cell::from(step.description.clone()),
                Cell::from(format!("{:.2}s", step.duration)),
                Cell::from(status_text).style(Style::default().fg(status_color)),
                Cell::from(step.error_count.to_string()),
                Cell::from(step.warning_count.to_string()),
            ])
        });

        let widths = [
            Constraint::Percentage(40),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(10),
        ];

        let title = " Summary ";

        let table = Table::new(rows, widths).header(header).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title(Title::from(title).alignment(Alignment::Center)),
        );

        frame.render_widget(table, area);
    }

    pub fn render_metrics(
        &self,
        frame: &mut Frame,
        area: Rect,
        metrics: &HashMap<String, MetricHistory>,
    ) {
        if metrics.is_empty() {
            let empty_msg = Paragraph::new("No metrics available")
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .title(" Runtime Metrics ")
                )
                .alignment(Alignment::Center);
            frame.render_widget(empty_msg, area);
            return;
        }

        let mut all_metrics: Vec<&MetricHistory> = metrics.values().collect();
        all_metrics.sort_by(|a, b| {
            a.category.cmp(&b.category).then(a.key.cmp(&b.key))
        });

        let metrics_per_row = 3;
        let total_metrics = all_metrics.len();
        let row_count = (total_metrics + metrics_per_row - 1) / metrics_per_row;

        if row_count == 0 {
            return;
        }

        let min_height_per_row = 8;
        let max_height_per_row = 12;

        let available_height = area.height;
        let ideal_total_height = row_count as u16 * min_height_per_row;

        let row_height = if ideal_total_height <= available_height {
            min_height_per_row.min(max_height_per_row)
        } else {
            (available_height / row_count as u16).max(6)
        };

        let constraints: Vec<Constraint> = (0..row_count)
            .map(|_| Constraint::Length(row_height))
            .collect();

        let row_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        for (row, chunk) in row_chunks.iter().enumerate().take(row_count) {
            let start_idx = row * metrics_per_row;
            let end_idx = (start_idx + metrics_per_row).min(total_metrics);
            let metrics_in_row = &all_metrics[start_idx..end_idx];

            if !metrics_in_row.is_empty() {
                self.render_metric_row(frame, *chunk, metrics_in_row);
            }
        }
    }

    fn render_metric_row(
        &self,
        frame: &mut Frame,
        area: Rect,
        metrics: &[&MetricHistory],
    ) {
        if metrics.is_empty() {
            return;
        }

        let metric_count = metrics.len();
        let constraints: Vec<Constraint> = (0..metric_count)
            .map(|_| Constraint::Ratio(1, metric_count as u32))
            .collect();

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(0)
            .constraints(constraints)
            .split(area);

        for (i, metric) in metrics.iter().enumerate() {
            if i < chunks.len() {
                self.render_single_metric(frame, chunks[i], metric);
            }
        }
    }

    fn render_single_metric(
        &self,
        frame: &mut Frame,
        area: Rect,
        metric: &MetricHistory,
    ) {
        if area.height < 4 || area.width < 12 {
            return;
        }

        match metric.visualization {
            MetricVisualization::Sparkline => {
                self.render_sparkline_metric(frame, area, metric)
            }
            MetricVisualization::Gauge => {
                self.render_percentage_metric(frame, area, metric)
            }
            MetricVisualization::Text => {
                self.render_simple_metric(frame, area, metric)
            }
            MetricVisualization::Chart => {
                self.render_sparkline_metric(frame, area, metric)
            }
            MetricVisualization::Bar => {
                self.render_sparkline_metric(frame, area, metric)
            }
            MetricVisualization::Table => {
                self.render_simple_metric(frame, area, metric)
            }
            MetricVisualization::Auto => {
                match metric.metric_type {
                    MetricType::FPS => self.render_fps_metric(frame, area, metric),
                    MetricType::Percentage => {
                        self.render_percentage_metric(frame, area, metric)
                    }
                    MetricType::TimeMillis => {
                        self.render_time_metric(frame, area, metric)
                    }
                    MetricType::Count | MetricType::Memory => {
                        self.render_sparkline_metric(frame, area, metric)
                    }
                    MetricType::Dimension | MetricType::Generic => {
                        self.render_simple_metric(frame, area, metric)
                    }
                }
            }
        }
    }

    fn render_fps_metric(
        &self,
        frame: &mut Frame,
        area: Rect,
        metric: &MetricHistory,
    ) {
        let latest = metric.latest_value().unwrap_or(0.0);
        let avg = metric.average().unwrap_or(0.0);

        let data: Vec<u64> = metric
            .values
            .iter()
            .map(|&v| v.round() as u64)
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Green))
            .title(format!(" {}: {} ", metric.category, metric.key));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height < 2 {
            return;
        }

        if inner.height >= 3 {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(inner);

            let sparkline = Sparkline::default()
                .data(&data)
                .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));

            frame.render_widget(sparkline, chunks[0]);

            let info = Paragraph::new(format!("{:.1} FPS (avg: {:.1})", latest, avg))
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center);

            frame.render_widget(info, chunks[1]);
        } else {
            let sparkline = Sparkline::default()
                .data(&data)
                .style(Style::default().fg(Color::Green).add_modifier(Modifier::BOLD));

            frame.render_widget(sparkline, inner);
        }
    }

    fn render_percentage_metric(
        &self,
        frame: &mut Frame,
        area: Rect,
        metric: &MetricHistory,
    ) {
        let latest = metric.latest_value().unwrap_or(0.0);
        let ratio = (latest / 100.0).clamp(0.0, 1.0);

        let gauge = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(format!(" {}: {} ", metric.category, metric.key)),
            )
            .gauge_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .ratio(ratio)
            .label(format!("{:.1}%", latest));

        frame.render_widget(gauge, area);
    }

    fn render_time_metric(
        &self,
        frame: &mut Frame,
        area: Rect,
        metric: &MetricHistory,
    ) {
        let latest = metric.latest_value().unwrap_or(0.0);
        let avg = metric.average().unwrap_or(0.0);
        let min = metric.min().unwrap_or(0.0);
        let max = metric.max().unwrap_or(0.0);

        let data: Vec<u64> = metric
            .values
            .iter()
            .map(|&v| (v * 10.0).round() as u64)
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Yellow))
            .title(format!(" {}: {} ", metric.category, metric.key));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height < 2 {
            return;
        }

        if inner.height >= 3 {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(inner);

            let sparkline = Sparkline::default()
                .data(&data)
                .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

            frame.render_widget(sparkline, chunks[0]);

            let info = Paragraph::new(format!(
                "{:.2}ms (avg: {:.2} min: {:.2} max: {:.2})",
                latest, avg, min, max
            ))
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);

            frame.render_widget(info, chunks[1]);
        } else {
            let sparkline = Sparkline::default()
                .data(&data)
                .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

            frame.render_widget(sparkline, inner);
        }
    }

    fn render_sparkline_metric(
        &self,
        frame: &mut Frame,
        area: Rect,
        metric: &MetricHistory,
    ) {
        let latest = metric.latest_value().unwrap_or(0.0);
        let avg = metric.average().unwrap_or(0.0);

        let data: Vec<u64> = metric
            .values
            .iter()
            .map(|&v| v.round() as u64)
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .title(format!(" {}: {} ", metric.category, metric.key));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height < 2 {
            return;
        }

        if inner.height >= 3 {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(inner);

            let sparkline = Sparkline::default()
                .data(&data)
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

            frame.render_widget(sparkline, chunks[0]);

            let info = Paragraph::new(format!("{:.0} (avg: {:.1})", latest, avg))
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center);

            frame.render_widget(info, chunks[1]);
        } else {
            let sparkline = Sparkline::default()
                .data(&data)
                .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

            frame.render_widget(sparkline, inner);
        }
    }

    fn render_simple_metric(
        &self,
        frame: &mut Frame,
        area: Rect,
        metric: &MetricHistory,
    ) {
        let latest = metric.latest_value().unwrap_or(0.0);

        let value_text = if latest.fract() == 0.0 {
            format!("{:.0}", latest)
        } else {
            format!("{:.2}", latest)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .title(format!(" {}: {} ", metric.category, metric.key));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height < 1 {
            return;
        }

        let paragraph = Paragraph::new(value_text)
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center);

        let center_y = inner.y + (inner.height.saturating_sub(1)) / 2;
        let text_area = Rect {
            x: inner.x,
            y: center_y,
            width: inner.width,
            height: 1.min(inner.height),
        };

        frame.render_widget(paragraph, text_area);
    }
}
