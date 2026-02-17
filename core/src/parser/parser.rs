use super::entry::{LogComponent, LogEntry, LogLevel};
use once_cell::sync::Lazy;
use regex::Regex;

static GCC_CLANG_ERROR: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([^:]+):(\d+):(\d+): error: (.+)$").unwrap());
static GCC_CLANG_WARNING: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([^:]+):(\d+):(\d+): warning: (.+)$").unwrap());
static GCC_CLANG_NOTE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^([^:]+):(\d+):(\d+): note: (.+)$").unwrap());
static CMAKE_ERROR: Lazy<Regex> = Lazy::new(|| Regex::new(r"^CMake Error").unwrap());
static CMAKE_WARNING: Lazy<Regex> = Lazy::new(|| Regex::new(r"^CMake Warning").unwrap());
static LINKER_ERROR_UNDEF: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"undefined reference to").unwrap());
static LINKER_ERROR_MULTI: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"multiple definition of").unwrap());

static BUILD_PROGRESS: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[(\d+)/(\d+)\]").unwrap());
static ANSI_ESCAPE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\x1b\[[0-9;]*m").unwrap());

#[derive(Clone)]
pub struct CompilerOutputParser {
    log_index: usize,
}

impl CompilerOutputParser {
    pub fn new() -> Self {
        Self { log_index: 0 }
    }

    pub fn parse_line(&mut self, line: &str) -> LogEntry {
        let stripped = strip_ansi(line);
        let index = self.log_index;
        self.log_index += 1;

        if let Some(caps) = GCC_CLANG_ERROR.captures(&stripped) {
            return LogEntry::new(
                LogLevel::Error,
                caps.get(4).unwrap().as_str().to_string(),
                line.to_string(),
                LogComponent::Compiler,
                index,
            )
            .with_location(
                caps.get(1).unwrap().as_str().to_string(),
                caps.get(2).and_then(|m| m.as_str().parse().ok()),
                caps.get(3).and_then(|m| m.as_str().parse().ok()),
            );
        }

        if let Some(caps) = GCC_CLANG_WARNING.captures(&stripped) {
            return LogEntry::new(
                LogLevel::Warning,
                caps.get(4).unwrap().as_str().to_string(),
                line.to_string(),
                LogComponent::Compiler,
                index,
            )
            .with_location(
                caps.get(1).unwrap().as_str().to_string(),
                caps.get(2).and_then(|m| m.as_str().parse().ok()),
                caps.get(3).and_then(|m| m.as_str().parse().ok()),
            );
        }

        if let Some(caps) = GCC_CLANG_NOTE.captures(&stripped) {
            return LogEntry::new(
                LogLevel::Debug,
                caps.get(4).unwrap().as_str().to_string(),
                line.to_string(),
                LogComponent::Compiler,
                index,
            )
            .with_location(
                caps.get(1).unwrap().as_str().to_string(),
                caps.get(2).and_then(|m| m.as_str().parse().ok()),
                caps.get(3).and_then(|m| m.as_str().parse().ok()),
            );
        }

        if CMAKE_ERROR.is_match(&stripped) {
            return LogEntry::new(
                LogLevel::Error,
                stripped.clone(),
                line.to_string(),
                LogComponent::CMake,
                index,
            );
        }

        if CMAKE_WARNING.is_match(&stripped) {
            return LogEntry::new(
                LogLevel::Warning,
                stripped.clone(),
                line.to_string(),
                LogComponent::CMake,
                index,
            );
        }

        if LINKER_ERROR_UNDEF.is_match(&stripped) || LINKER_ERROR_MULTI.is_match(&stripped) {
            return LogEntry::new(
                LogLevel::Error,
                stripped.clone(),
                line.to_string(),
                LogComponent::Linker,
                index,
            );
        }

        if BUILD_PROGRESS.is_match(&stripped) {
            return LogEntry::new(
                LogLevel::Info,
                stripped.clone(),
                line.to_string(),
                LogComponent::Build,
                index,
            )
            .with_tags(vec!["progress".to_string()]);
        }

        LogEntry::new(
            LogLevel::Info,
            stripped,
            line.to_string(),
            LogComponent::Other("unknown".to_string()),
            index,
        )
    }

    pub fn reset(&mut self) {
        self.log_index = 0;
    }
}

impl Default for CompilerOutputParser {
    fn default() -> Self {
        Self::new()
    }
}

fn strip_ansi(s: &str) -> String {
    ANSI_ESCAPE.replace_all(s, "").to_string()
}

pub struct MetricParser;

impl MetricParser {
    pub fn parse_metric_line(line: &str) -> Option<crate::executor::RuntimeMetric> {
        use crate::executor::MetricVisualization;
        use std::time::Instant;

        if let Some(metric_str) = line.strip_prefix("[IGNIS_METRIC] ") {
            let parts: Vec<&str> = metric_str.split(':').collect();

            if parts.len() >= 2 {
                let category = parts[0].to_string();
                let kv: Vec<&str> = parts[1].split('=').collect();
                if kv.len() == 2 {
                    let key = kv[0].to_string();
                    let value_and_viz: Vec<&str> = kv[1].split(':').collect();

                    let value = value_and_viz[0].to_string();
                    let explicit_visualization = if value_and_viz.len() > 1 {
                        MetricVisualization::from_str(value_and_viz[1])
                    } else {
                        None
                    };

                    return Some(crate::executor::RuntimeMetric {
                        key,
                        value,
                        timestamp: Instant::now(),
                        category,
                        explicit_visualization,
                    });
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcc_error_parsing() {
        let mut parser = CompilerOutputParser::new();
        let line = "src/main.cpp:42:10: error: 'foo' was not declared in this scope";
        let entry = parser.parse_line(line);

        assert_eq!(entry.level, LogLevel::Error);
        assert_eq!(entry.component, LogComponent::Compiler);
        assert_eq!(entry.file_path, Some("src/main.cpp".to_string()));
        assert_eq!(entry.line_number, Some(42));
        assert_eq!(entry.column, Some(10));
    }

    #[test]
    fn test_cmake_error_parsing() {
        let mut parser = CompilerOutputParser::new();
        let line = "CMake Error: Some configuration error";
        let entry = parser.parse_line(line);

        assert_eq!(entry.level, LogLevel::Error);
        assert_eq!(entry.component, LogComponent::CMake);
    }

    #[test]
    fn test_ansi_stripping() {
        let ansi_str = "\x1b[31mError:\x1b[0m Something went wrong";
        let stripped = strip_ansi(ansi_str);
        assert_eq!(stripped, "Error: Something went wrong");
    }
}
