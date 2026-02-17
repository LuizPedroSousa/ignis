use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
    Fatal,
}

impl LogLevel {
    pub fn to_str(&self) -> &'static str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARNING",
            LogLevel::Error => "ERROR",
            LogLevel::Fatal => "FATAL",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogComponent {
    CMake,
    Compiler,
    Linker,
    Build,
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Local>,
    pub level: LogLevel,
    pub message: String,
    pub raw_line: String,
    pub file_path: Option<String>,
    pub line_number: Option<usize>,
    pub column: Option<usize>,
    pub component: LogComponent,
    pub tags: Vec<String>,
    pub index: usize,
}

impl LogEntry {
    pub fn new(
        level: LogLevel,
        message: String,
        raw_line: String,
        component: LogComponent,
        index: usize,
    ) -> Self {
        Self {
            timestamp: Local::now(),
            level,
            message,
            raw_line,
            file_path: None,
            line_number: None,
            column: None,
            component,
            tags: Vec::new(),
            index,
        }
    }

    pub fn with_location(
        mut self,
        file_path: String,
        line_number: Option<usize>,
        column: Option<usize>,
    ) -> Self {
        self.file_path = Some(file_path);
        self.line_number = line_number;
        self.column = column;
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn location_string(&self) -> Option<String> {
        self.file_path.as_ref().map(|path| {
            let mut loc = path.clone();
            if let Some(line) = self.line_number {
                loc.push(':');
                loc.push_str(&line.to_string());
                if let Some(col) = self.column {
                    loc.push(':');
                    loc.push_str(&col.to_string());
                }
            }
            loc
        })
    }
}
