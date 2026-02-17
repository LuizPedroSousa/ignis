use crate::history::BuildStepResult;
use crate::monitor::ResourceStats;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    symbols,
    text::{Line, Span},
    widgets::{
        block::Title, Axis, Block, BorderType, Borders, Chart, Dataset, GraphType, Paragraph,
    },
    Frame,
};

pub struct PerformanceTab<'a> {
    steps: &'a [BuildStepResult],
    total_duration: f64,
    build_complete: bool,
    resource_stats: ResourceStats,
}

impl<'a> PerformanceTab<'a> {
    pub fn new(
        steps: &'a [BuildStepResult],
        total_duration: f64,
        build_complete: bool,
        resource_stats: ResourceStats,
    ) -> Self {
        Self {
            steps,
            total_duration,
            build_complete,
            resource_stats,
        }
    }

    pub fn new_runtime(
        total_duration: f64,
        exec_complete: bool,
        resource_stats: ResourceStats,
    ) -> Self {
        Self {
            steps: &[],
            total_duration,
            build_complete: exec_complete,
            resource_stats,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if self.steps.is_empty() {
            self.render_resource_usage(frame, area);
        } else {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
                .split(area);

            self.render_build_metrics(frame, chunks[0]);
            self.render_resource_usage(frame, chunks[1]);
        }
    }

    fn render_build_metrics(&self, frame: &mut Frame, area: Rect) {
        use std::collections::HashMap;

        let steps_total: f64 = self.steps.iter().map(|s| s.duration).sum();
        let total_errors: usize = self.steps.iter().map(|s| s.error_count).sum();
        let total_warnings: usize = self.steps.iter().map(|s| s.warning_count).sum();
        let success_count = self.steps.iter().filter(|s| s.success).count();

        let efficiency = if self.total_duration > 0.0 {
            (steps_total / self.total_duration) * 100.0
        } else {
            0.0
        };

        let overhead = self.total_duration - steps_total;
        let mode_indicator = if self.build_complete { "" } else { " (Live)" };

        let mut grouped_steps: HashMap<String, Vec<&BuildStepResult>> = HashMap::new();
        for step in self.steps.iter() {
            let step_type = self.extract_step_type(&step.description);
            std::collections::hash_map::Entry::or_insert_with(
                grouped_steps.entry(step_type),
                Vec::new,
            )
            .push(step);
        }

        let step_order = ["Configure", "Build", "Install", "Other"];
        let step_types: Vec<String> = step_order
            .iter()
            .filter(|&st| grouped_steps.contains_key(*st))
            .map(|s| s.to_string())
            .collect();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(6), Constraint::Min(0)])
            .split(area);

        let summary_lines = vec![
            Line::from(vec![
                Span::styled("Total: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{:.2}s", self.total_duration),
                    Style::default().fg(Color::Green),
                ),
                Span::styled(mode_indicator, Style::default().fg(Color::Cyan)),
                Span::raw("  "),
                Span::styled("│ ", Style::default().fg(Color::DarkGray)),
                Span::styled("Efficiency: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{:.1}%", efficiency),
                    Style::default().fg(if efficiency > 90.0 {
                        Color::Green
                    } else if efficiency > 70.0 {
                        Color::Yellow
                    } else {
                        Color::Red
                    }),
                ),
                Span::raw("  "),
                Span::styled("│ ", Style::default().fg(Color::DarkGray)),
                Span::styled("Overhead: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{:.2}s", overhead),
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
            Line::from(vec![
                Span::styled("Steps: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{}/{}", success_count, self.steps.len()),
                    Style::default().fg(if success_count == self.steps.len() {
                        Color::Green
                    } else {
                        Color::Red
                    }),
                ),
                Span::raw("  "),
                Span::styled("│ ", Style::default().fg(Color::DarkGray)),
                Span::styled("Errors: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{}", total_errors),
                    Style::default().fg(if total_errors > 0 {
                        Color::Red
                    } else {
                        Color::Green
                    }),
                ),
                Span::raw("  "),
                Span::styled("│ ", Style::default().fg(Color::DarkGray)),
                Span::styled("Warnings: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{}", total_warnings),
                    Style::default().fg(if total_warnings > 0 {
                        Color::Yellow
                    } else {
                        Color::Green
                    }),
                ),
            ]),
        ];

        let summary = Paragraph::new(summary_lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title(Title::from(" Build Summary ").alignment(Alignment::Center)),
        );

        frame.render_widget(summary, chunks[0]);

        let step_count = step_types.len().max(1);
        let step_constraints: Vec<Constraint> = (0..step_count)
            .map(|_| Constraint::Ratio(1, step_count as u32))
            .collect();

        let step_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(step_constraints)
            .split(chunks[1]);

        for (idx, step_type) in step_types.iter().enumerate() {
            let steps = grouped_steps.get(step_type).unwrap();
            self.render_step_type_panel(frame, step_chunks[idx], step_type, steps);
        }
    }

    fn extract_step_type(&self, description: &str) -> String {
        if description.starts_with("Configuring") || description.contains("CMake") {
            "Configure".to_string()
        } else if description.starts_with("Building") {
            "Build".to_string()
        } else if description.starts_with("Installing") {
            "Install".to_string()
        } else {
            "Other".to_string()
        }
    }

    fn render_step_type_panel(
        &self,
        frame: &mut Frame,
        area: Rect,
        step_type: &str,
        steps: &[&BuildStepResult],
    ) {
        let max_duration = steps.iter().map(|s| s.duration).fold(0.0f64, f64::max);
        let bar_width = (area.width.saturating_sub(15)).min(25) as usize;

        let mut lines = vec![];

        for step in steps.iter() {
            let project_name = self.extract_project_name(&step.description);

            let bar = if max_duration > 0.0 {
                self.render_progress_bar(step.duration, max_duration, bar_width)
            } else {
                self.render_progress_bar(0.0, 1.0, bar_width)
            };

            let status_icon = if step.success { "✓" } else { "✗" };
            let status_color = if step.success {
                Color::Green
            } else {
                Color::Red
            };

            lines.push(Line::from(vec![
                Span::styled(status_icon, Style::default().fg(status_color)),
                Span::raw(" "),
                Span::styled(project_name, Style::default().fg(Color::White)),
            ]));

            lines.push(Line::from(vec![Span::styled(
                format!("  {:.2}s", step.duration),
                Style::default().fg(Color::Cyan),
            )]));

            let bar_color = if step.error_count > 0 {
                Color::Red
            } else if step.warning_count > 0 {
                Color::Yellow
            } else if self.build_complete {
                Color::Blue
            } else {
                Color::Magenta
            };

            lines.push(Line::from(Span::styled(
                format!("  {}", bar),
                Style::default().fg(bar_color),
            )));

            if step.error_count > 0 || step.warning_count > 0 {
                let mut parts = vec![];
                if step.error_count > 0 {
                    parts.push(format!("{}E", step.error_count));
                }
                if step.warning_count > 0 {
                    parts.push(format!("{}W", step.warning_count));
                }
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(parts.join(" "), Style::default().fg(Color::Yellow)),
                ]));
            }

            lines.push(Line::from(""));
        }

        let type_duration: f64 = steps.iter().map(|s| s.duration).sum();
        let type_errors: usize = steps.iter().map(|s| s.error_count).sum();
        let type_warnings: usize = steps.iter().map(|s| s.warning_count).sum();

        let title = format!(" {} ({:.1}s) ", step_type, type_duration);

        let title_style = if type_errors > 0 {
            Style::default().fg(Color::Red)
        } else if type_warnings > 0 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Green)
        };

        let paragraph = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(title_style)
                .title(Title::from(title).alignment(Alignment::Center)),
        );

        frame.render_widget(paragraph, area);
    }

    fn extract_project_name(&self, description: &str) -> String {
        if description.starts_with("Configuring ") {
            description
                .strip_prefix("Configuring ")
                .unwrap_or(description)
                .to_string()
        } else if description.starts_with("Building ") {
            description
                .strip_prefix("Building ")
                .unwrap_or(description)
                .to_string()
        } else if description.starts_with("Installing ") {
            description
                .strip_prefix("Installing ")
                .unwrap_or(description)
                .to_string()
        } else {
            description.to_string()
        }
    }

    fn render_resource_usage(&self, frame: &mut Frame, area: Rect) {
        if !self.resource_stats.samples.is_empty() {
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(10),
                    Constraint::Length(15),
                    Constraint::Length(15),
                ])
                .split(area);

            let horizontal_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(main_chunks[0]);

            self.render_cpu_chart(frame, main_chunks[1]);
            self.render_memory_chart(frame, main_chunks[2]);

            self.render_left_metrics(frame, horizontal_chunks[0]);
            self.render_right_metrics(frame, horizontal_chunks[1]);
        } else {
            let horizontal_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(area);

            self.render_left_metrics(frame, horizontal_chunks[0]);
            self.render_right_metrics(frame, horizontal_chunks[1]);
        }
    }

    fn render_left_metrics(&self, frame: &mut Frame, area: Rect) {
        let bar_width = (area.width.saturating_sub(10)).min(30) as usize;

        let cpu_bar =
            self.render_progress_bar(self.resource_stats.peak_cpu as f64, 100.0, bar_width);
        let mem_bar =
            self.render_progress_bar(self.resource_stats.peak_memory_mb, 2000.0, bar_width);
        let thread_bar =
            self.render_progress_bar(self.resource_stats.peak_threads as f64, 64.0, bar_width);

        let lines = vec![
            Line::from(vec![
                Span::styled("Peak CPU: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{:.1}%", self.resource_stats.peak_cpu),
                    Style::default().fg(if self.resource_stats.peak_cpu > 80.0 {
                        Color::Red
                    } else if self.resource_stats.peak_cpu > 50.0 {
                        Color::Yellow
                    } else {
                        Color::Green
                    }),
                ),
            ]),
            Line::from(vec![
                Span::styled("Avg CPU:  ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{:.1}%", self.resource_stats.avg_cpu),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![Span::styled(
                cpu_bar,
                Style::default().fg(if self.resource_stats.peak_cpu > 80.0 {
                    Color::Red
                } else if self.resource_stats.peak_cpu > 50.0 {
                    Color::Yellow
                } else {
                    Color::Green
                }),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Peak Mem: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{:.1} MB", self.resource_stats.peak_memory_mb),
                    Style::default().fg(if self.resource_stats.peak_memory_mb > 1000.0 {
                        Color::Red
                    } else if self.resource_stats.peak_memory_mb > 500.0 {
                        Color::Yellow
                    } else {
                        Color::Green
                    }),
                ),
            ]),
            Line::from(vec![
                Span::styled("Avg Mem:  ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{:.1} MB", self.resource_stats.avg_memory_mb),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![Span::styled(
                mem_bar,
                Style::default().fg(if self.resource_stats.peak_memory_mb > 1000.0 {
                    Color::Red
                } else if self.resource_stats.peak_memory_mb > 500.0 {
                    Color::Yellow
                } else {
                    Color::Green
                }),
            )]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Peak Thr: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{}", self.resource_stats.peak_threads),
                    Style::default().fg(if self.resource_stats.peak_threads > 32 {
                        Color::Red
                    } else if self.resource_stats.peak_threads > 16 {
                        Color::Yellow
                    } else {
                        Color::Green
                    }),
                ),
            ]),
            Line::from(vec![
                Span::styled("Avg Thr:  ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{:.1}", self.resource_stats.avg_threads),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![Span::styled(
                thread_bar,
                Style::default().fg(if self.resource_stats.peak_threads > 32 {
                    Color::Red
                } else if self.resource_stats.peak_threads > 16 {
                    Color::Yellow
                } else {
                    Color::Green
                }),
            )]),
        ];

        let paragraph = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title(Title::from(" CPU / Memory / Threads ").alignment(Alignment::Center)),
        );

        frame.render_widget(paragraph, area);
    }

    fn render_right_metrics(&self, frame: &mut Frame, area: Rect) {
        let bar_width = (area.width.saturating_sub(10)).min(30) as usize;

        let disk_total =
            self.resource_stats.total_disk_read_mb + self.resource_stats.total_disk_write_mb;
        let read_ratio = if disk_total > 0.0 {
            self.resource_stats.total_disk_read_mb / disk_total
        } else {
            0.5
        };
        let disk_bar = self.render_dual_progress_bar(read_ratio, bar_width);
        let load_bar = self.render_progress_bar(self.resource_stats.load_avg_1min, 8.0, bar_width);

        let mut lines = vec![
            Line::from(vec![
                Span::styled("Disk Read:  ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{:.2} MB", self.resource_stats.total_disk_read_mb),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![
                Span::styled("Disk Write: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{:.2} MB", self.resource_stats.total_disk_write_mb),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![
                Span::styled(disk_bar.0, Style::default().fg(Color::Blue)),
                Span::styled(disk_bar.1, Style::default().fg(Color::Magenta)),
            ]),
            Line::from(vec![
                Span::styled("Read ", Style::default().fg(Color::Blue)),
                Span::styled("━", Style::default().fg(Color::Blue)),
                Span::raw(" "),
                Span::styled("Write ", Style::default().fg(Color::Magenta)),
                Span::styled("━", Style::default().fg(Color::Magenta)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Load Avg: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{:.2}", self.resource_stats.load_avg_1min),
                    Style::default().fg(if self.resource_stats.load_avg_1min > 4.0 {
                        Color::Red
                    } else if self.resource_stats.load_avg_1min > 2.0 {
                        Color::Yellow
                    } else {
                        Color::Green
                    }),
                ),
            ]),
            Line::from(vec![Span::styled(
                load_bar,
                Style::default().fg(if self.resource_stats.load_avg_1min > 4.0 {
                    Color::Red
                } else if self.resource_stats.load_avg_1min > 2.0 {
                    Color::Yellow
                } else {
                    Color::Green
                }),
            )]),
        ];

        if !self.resource_stats.samples.is_empty() {
            let sample_count = self.resource_stats.samples.len();
            let time_range = if sample_count > 1 {
                self.resource_stats.samples.last().unwrap().timestamp
                    - self.resource_stats.samples[0].timestamp
            } else {
                0.0
            };

            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Samples:",
                Style::default().fg(Color::Yellow),
            )));
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{}", sample_count),
                    Style::default().fg(Color::Cyan),
                ),
                Span::raw(" over "),
                Span::styled(
                    format!("{:.1}s", time_range),
                    Style::default().fg(Color::Cyan),
                ),
            ]));
        } else {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Collecting data...",
                Style::default().fg(Color::DarkGray),
            )));
        }

        let paragraph = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title(Title::from(" Disk / Load / Samples ").alignment(Alignment::Center)),
        );

        frame.render_widget(paragraph, area);
    }

    fn render_cpu_chart(&self, frame: &mut Frame, area: Rect) {
        if self.resource_stats.samples.is_empty() {
            return;
        }

        let start_time = self.resource_stats.samples[0].timestamp;
        let cpu_data: Vec<(f64, f64)> = self
            .resource_stats
            .samples
            .iter()
            .map(|s| (s.timestamp - start_time, s.cpu_usage as f64))
            .collect();

        let datasets = vec![Dataset::default()
            .name("CPU %")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Green))
            .data(&cpu_data)];

        let x_max = cpu_data.last().map(|(x, _)| *x).unwrap_or(0.0);

        let y_min = cpu_data
            .iter()
            .map(|(_, y)| *y)
            .fold(f64::INFINITY, f64::min)
            .max(0.0);
        let y_max = cpu_data
            .iter()
            .map(|(_, y)| *y)
            .fold(0.0f64, f64::max)
            .max(10.0);

        let y_range = y_max - y_min;
        let y_padding = if y_range < 1.0 { 5.0 } else { y_range * 0.2 };
        let y_min_bound = (y_min - y_padding).max(0.0);
        let y_max_bound = y_max + y_padding;

        let x_labels = vec![
            Span::raw("0s"),
            Span::raw(format!("{:.1}s", x_max / 2.0)),
            Span::raw(format!("{:.1}s", x_max)),
        ];

        let y_labels = vec![
            Span::raw(format!("{:.1}", y_min_bound)),
            Span::raw(format!("{:.1}", (y_min_bound + y_max_bound) / 2.0)),
            Span::raw(format!("{:.1}", y_max_bound)),
        ];

        let chart = Chart::new(datasets)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(Title::from(" CPU Usage (%) ").alignment(Alignment::Center)),
            )
            .x_axis(
                Axis::default()
                    .title("Time")
                    .style(Style::default().fg(Color::Gray))
                    .bounds([0.0, x_max])
                    .labels(x_labels),
            )
            .y_axis(
                Axis::default()
                    .title("%")
                    .style(Style::default().fg(Color::Gray))
                    .bounds([y_min_bound, y_max_bound])
                    .labels(y_labels),
            );

        frame.render_widget(chart, area);
    }

    fn render_memory_chart(&self, frame: &mut Frame, area: Rect) {
        if self.resource_stats.samples.is_empty() {
            return;
        }

        let start_time = self.resource_stats.samples[0].timestamp;
        let mem_data: Vec<(f64, f64)> = self
            .resource_stats
            .samples
            .iter()
            .map(|s| (s.timestamp - start_time, s.memory_mb))
            .collect();

        let datasets = vec![Dataset::default()
            .name("Memory MB")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Yellow))
            .data(&mem_data)];

        let x_max = mem_data.last().map(|(x, _)| *x).unwrap_or(0.0);

        let y_min = mem_data
            .iter()
            .map(|(_, y)| *y)
            .fold(f64::INFINITY, f64::min)
            .max(0.0);
        let y_max = mem_data
            .iter()
            .map(|(_, y)| *y)
            .fold(0.0f64, f64::max)
            .max(10.0);

        let y_range = y_max - y_min;
        let y_padding = if y_range < 1.0 { 10.0 } else { y_range * 0.2 };
        let y_min_bound = (y_min - y_padding).max(0.0);
        let y_max_bound = y_max + y_padding;

        let x_labels = vec![
            Span::raw("0s"),
            Span::raw(format!("{:.1}s", x_max / 2.0)),
            Span::raw(format!("{:.1}s", x_max)),
        ];

        let y_labels = vec![
            Span::raw(format!("{:.0}", y_min_bound)),
            Span::raw(format!("{:.0}", (y_min_bound + y_max_bound) / 2.0)),
            Span::raw(format!("{:.0}", y_max_bound)),
        ];

        let chart = Chart::new(datasets)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(Title::from(" Memory Usage (MB) ").alignment(Alignment::Center)),
            )
            .x_axis(
                Axis::default()
                    .title("Time")
                    .style(Style::default().fg(Color::Gray))
                    .bounds([0.0, x_max])
                    .labels(x_labels),
            )
            .y_axis(
                Axis::default()
                    .title("MB")
                    .style(Style::default().fg(Color::Gray))
                    .bounds([y_min_bound, y_max_bound])
                    .labels(y_labels),
            );

        frame.render_widget(chart, area);
    }

    fn render_progress_bar(&self, value: f64, max: f64, width: usize) -> String {
        let percentage = (value / max).min(1.0);
        let filled = (percentage * width as f64) as usize;
        let empty = width.saturating_sub(filled);

        format!("{}{}", "█".repeat(filled), "░".repeat(empty))
    }

    fn render_dual_progress_bar(&self, ratio: f64, width: usize) -> (String, String) {
        let first_width = (ratio * width as f64) as usize;
        let second_width = width.saturating_sub(first_width);

        ("█".repeat(first_width), "█".repeat(second_width))
    }
}
