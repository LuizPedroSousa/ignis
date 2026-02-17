use anyhow::Context;
use std::collections::VecDeque;
use std::process::Stdio;
use std::time::Instant;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::builder::BuildStep;

#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub success: bool,
    pub duration: f64,
    pub stdout: Vec<String>,
    pub stderr: Vec<String>,
    pub exit_code: Option<i32>,
    pub failure_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeMetric {
    pub key: String,
    pub value: String,
    pub timestamp: Instant,
    pub category: String,
    pub explicit_visualization: Option<MetricVisualization>,
}

impl RuntimeMetric {
    pub fn parse_numeric_value(&self) -> Option<f64> {
        self.value.parse::<f64>().ok()
    }

    pub fn visualization(&self) -> MetricVisualization {
        if let Some(viz) = self.explicit_visualization {
            return viz;
        }

        match self.metric_type() {
            MetricType::FPS | MetricType::TimeMillis | MetricType::Count | MetricType::Memory => {
                MetricVisualization::Sparkline
            }
            MetricType::Percentage => MetricVisualization::Gauge,
            MetricType::Dimension | MetricType::Generic => MetricVisualization::Text,
        }
    }

    pub fn metric_type(&self) -> MetricType {
        let lower_key = self.key.to_lowercase();

        if lower_key.contains("fps") {
            return MetricType::FPS;
        }
        if lower_key.contains("percent")
            || lower_key.contains("percentage")
            || lower_key.ends_with("_pct")
        {
            return MetricType::Percentage;
        }
        if lower_key.contains("time") && lower_key.contains("ms") {
            return MetricType::TimeMillis;
        }
        if lower_key.contains("count")
            || lower_key.contains("entities")
            || lower_key.contains("draw_calls")
        {
            return MetricType::Count;
        }
        if lower_key.contains("mb") || lower_key.contains("memory") || lower_key.contains("heap") {
            return MetricType::Memory;
        }
        if lower_key.contains("width") || lower_key.contains("height") {
            return MetricType::Dimension;
        }

        MetricType::Generic
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricType {
    FPS,
    Percentage,
    TimeMillis,
    Count,
    Memory,
    Dimension,
    Generic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricVisualization {
    Sparkline,
    Gauge,
    Table,
    Chart,
    Bar,
    Text,
    Auto,
}

impl MetricVisualization {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "sparkline" => Some(MetricVisualization::Sparkline),
            "gauge" => Some(MetricVisualization::Gauge),
            "table" => Some(MetricVisualization::Table),
            "chart" => Some(MetricVisualization::Chart),
            "bar" => Some(MetricVisualization::Bar),
            "text" => Some(MetricVisualization::Text),
            "auto" => Some(MetricVisualization::Auto),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MetricHistory {
    pub category: String,
    pub key: String,
    pub values: VecDeque<f64>,
    pub timestamps: VecDeque<Instant>,
    pub metric_type: MetricType,
    pub visualization: MetricVisualization,
    pub max_history: usize,
}

impl MetricHistory {
    pub fn new(
        category: String,
        key: String,
        metric_type: MetricType,
        visualization: MetricVisualization,
    ) -> Self {
        Self {
            category,
            key,
            values: VecDeque::with_capacity(50),
            timestamps: VecDeque::with_capacity(50),
            metric_type,
            visualization,
            max_history: 50,
        }
    }

    pub fn add_value(&mut self, value: f64, timestamp: Instant) {
        if self.values.len() >= self.max_history {
            self.values.pop_front();
            self.timestamps.pop_front();
        }
        self.values.push_back(value);
        self.timestamps.push_back(timestamp);
    }

    pub fn latest_value(&self) -> Option<f64> {
        self.values.back().copied()
    }

    pub fn latest_timestamp(&self) -> Option<Instant> {
        self.timestamps.back().copied()
    }

    pub fn average(&self) -> Option<f64> {
        if self.values.is_empty() {
            return None;
        }
        let sum: f64 = self.values.iter().sum();
        Some(sum / self.values.len() as f64)
    }

    pub fn min(&self) -> Option<f64> {
        self.values
            .iter()
            .copied()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
    }

    pub fn max(&self) -> Option<f64> {
        self.values
            .iter()
            .copied()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
    }
}

#[derive(Debug, Clone)]
pub enum StepUpdate {
    Started(String),
    Progress(String),
    Finished(ExecutionResult),
    ProcessStarted(u32),
    ProcessFinished(u32),
    Metric(RuntimeMetric),
}

pub async fn execute_step<F>(
    command: Vec<String>,
    mut output_callback: F,
    step_callback: Option<&mpsc::UnboundedSender<StepUpdate>>,
) -> anyhow::Result<ExecutionResult>
where
    F: FnMut(String) + Send + 'static,
{
    let start = Instant::now();

    let program = &command[0];
    let args = &command[1..];

    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to spawn command: {}", program))?;

    let pid = child.id();
    if let (Some(pid), Some(callback)) = (pid, step_callback) {
        callback
            .send(StepUpdate::ProcessStarted(pid))
            .expect("Failed to send ProcessStarted update");
    }

    let stdout = child.stdout.take().context("Failed to capture stdout")?;
    let stderr = child.stderr.take().context("Failed to capture stderr")?;

    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    let tx_clone = tx.clone();
    let stdout_task = tokio::spawn(async move {
        let mut lines = Vec::new();
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            lines.push(line.clone());
            tx_clone
                .send(line)
                .expect("Failed to send stdout line to channel");
        }
        lines
    });

    let stderr_task = tokio::spawn(async move {
        let mut lines = Vec::new();
        let mut reader = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            lines.push(line.clone());
            tx.send(line)
                .expect("Failed to send stderr line to channel");
        }
        lines
    });

    let callback_task = tokio::spawn(async move {
        while let Some(line) = rx.recv().await {
            output_callback(line);
        }
    });

    let status = child
        .wait()
        .await
        .context("Failed to wait for child process")?;

    if let (Some(pid), Some(callback)) = (pid, step_callback) {
        callback
            .send(StepUpdate::ProcessFinished(pid))
            .expect("Failed to send ProcessFinished update");
    }

    let stdout_lines = stdout_task.await.context("stdout task panicked")?;
    let stderr_lines = stderr_task.await.context("stderr task panicked")?;

    callback_task.abort();

    let duration = start.elapsed().as_secs_f64();
    let exit_code = status.code();
    let failure_reason = if !status.success() {
        exit_code.map(|code| format!("Exit code {}", code))
    } else {
        None
    };

    Ok(ExecutionResult {
        success: status.success(),
        duration,
        stdout: stdout_lines,
        stderr: stderr_lines,
        exit_code,
        failure_reason,
    })
}

pub async fn execute_steps<F>(
    steps: Vec<BuildStep>,
    output_callback: F,
    step_callback: mpsc::UnboundedSender<StepUpdate>,
) -> anyhow::Result<Vec<ExecutionResult>>
where
    F: FnMut(String) + Send + 'static + Clone,
{
    let mut results = Vec::new();

    for step in steps {
        step_callback
            .send(StepUpdate::Started(step.description.clone()))
            .expect("Failed to send step Started update");

        let callback = output_callback.clone();
        let result = execute_step(step.commands, callback, Some(&step_callback)).await?;

        step_callback
            .send(StepUpdate::Finished(result.clone()))
            .expect("Failed to send step Finished update");

        let success = result.success;
        results.push(result);

        if !success {
            break;
        }
    }

    Ok(results)
}

pub async fn execute_program(
    exec_info: crate::builder::ExecutableInfo,
    log_tx: mpsc::UnboundedSender<crate::parser::entry::LogEntry>,
    step_tx: mpsc::UnboundedSender<StepUpdate>,
) -> anyhow::Result<ExecutionResult> {
    use crate::parser::entry::{LogComponent, LogEntry, LogLevel};
    use crate::parser::parser::MetricParser;

    let start = Instant::now();

    let mut child = Command::new(&exec_info.path)
        .current_dir(&exec_info.project_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to spawn executable: {}", exec_info.path.display()))?;

    let pid = child.id();
    if let Some(pid) = pid {
        step_tx
            .send(StepUpdate::ProcessStarted(pid))
            .expect("Failed to send program ProcessStarted update");
    }

    let stdout = child.stdout.take().context("Failed to capture stdout")?;
    let stderr = child.stderr.take().context("Failed to capture stderr")?;

    let log_tx_stdout = log_tx.clone();
    let step_tx_clone = step_tx.clone();

    let stdout_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        let mut index = 0;
        while let Ok(Some(line)) = reader.next_line().await {
            if let Some(metric) = MetricParser::parse_metric_line(&line) {
                step_tx_clone
                    .send(StepUpdate::Metric(metric))
                    .expect("Failed to send Metric update");
            } else {
                let entry = LogEntry::new(
                    LogLevel::Info,
                    line.clone(),
                    line.clone(),
                    LogComponent::Other("exec".to_string()),
                    index,
                );
                index += 1;
                log_tx_stdout
                    .send(entry)
                    .expect("Failed to send stdout log entry");
            }
        }
    });

    let log_tx_stderr = log_tx.clone();
    let stderr_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stderr).lines();
        let mut index = 100000;
        while let Ok(Some(line)) = reader.next_line().await {
            let entry = LogEntry::new(
                LogLevel::Error,
                line.clone(),
                line.clone(),
                LogComponent::Other("exec".to_string()),
                index,
            );
            index += 1;
            log_tx_stderr
                .send(entry)
                .expect("Failed to send stderr log entry");
        }
    });

    let status = child
        .wait()
        .await
        .context("Failed to wait for child process")?;

    stdout_task.await.context("stdout task panicked")?;
    stderr_task.await.context("stderr task panicked")?;

    let duration = start.elapsed().as_secs_f64();
    let exit_code = status.code();
    let mut failure_reason: Option<String> = None;

    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;

        if let Some(signal) = status.signal() {
            let signal_name = match signal {
                1 => "SIGHUP (Hangup)",
                2 => "SIGINT (Interrupt)",
                3 => "SIGQUIT (Quit)",
                4 => "SIGILL (Illegal instruction)",
                6 => "SIGABRT (Abort)",
                8 => "SIGFPE (Floating point exception)",
                9 => "SIGKILL (Killed)",
                11 => "SIGSEGV (Segmentation fault)",
                13 => "SIGPIPE (Broken pipe)",
                15 => "SIGTERM (Terminated)",
                _ => "Unknown signal",
            };

            failure_reason = Some(format!("Signal {} ({})", signal, signal_name));

            let message = format!(
                "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\
                                   Process terminated by signal {}\n\
                                   Signal: {}\n\
                                   ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━",
                signal, signal_name
            );

            let error_entry = LogEntry::new(
                LogLevel::Error,
                message.clone(),
                message,
                LogComponent::Other("system".to_string()),
                999999,
            );
            log_tx
                .send(error_entry)
                .expect("Failed to send signal error log entry");
        }
    }

    #[cfg(not(unix))]
    {
        if exit_code.is_none() && !status.success() {
            failure_reason = Some("Abnormal termination".to_string());
            let message = "Process terminated abnormally (no exit code)".to_string();
            let error_entry = LogEntry::new(
                LogLevel::Error,
                message.clone(),
                message,
                LogComponent::Other("system".to_string()),
                999999,
            );
            log_tx
                .send(error_entry)
                .expect("Failed to send abnormal termination log entry");
        }
    }

    if let Some(code) = exit_code {
        if code != 0 {
            if failure_reason.is_none() {
                failure_reason = Some(format!("Exit code {}", code));
            }
            let message = format!("Process exited with code: {}", code);
            let info_entry = LogEntry::new(
                LogLevel::Warning,
                message.clone(),
                message,
                LogComponent::Other("system".to_string()),
                999998,
            );
            log_tx
                .send(info_entry)
                .expect("Failed to send exit code log entry");
        }
    }

    let result = ExecutionResult {
        success: status.success(),
        duration,
        stdout: vec![],
        stderr: vec![],
        exit_code,
        failure_reason,
    };

    step_tx
        .send(StepUpdate::Finished(result.clone()))
        .expect("Failed to send program Finished update");

    if let Some(pid) = pid {
        step_tx
            .send(StepUpdate::ProcessFinished(pid))
            .expect("Failed to send program ProcessFinished update");
    }

    Ok(result)
}
