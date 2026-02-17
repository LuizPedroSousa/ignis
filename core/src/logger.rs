use crate::parser::entry::{LogEntry, LogLevel};
use std::io::Write;

pub struct Logger {
    use_colors: bool,
}

impl Logger {
    pub fn new() -> Self {
        Self {
            use_colors: atty::is(atty::Stream::Stdout),
        }
    }

    pub fn log_entry(&self, entry: &LogEntry) {
        if self.use_colors {
            self.log_colored(entry);
        } else {
            self.log_plain(entry);
        }
    }

    pub fn log(&self, level: LogLevel, message: &str) {
        if self.use_colors {
            let color = match level {
                LogLevel::Debug => "\x1b[90m",
                LogLevel::Info => "\x1b[37m",
                LogLevel::Warning => "\x1b[33m",
                LogLevel::Error => "\x1b[31m",
                LogLevel::Fatal => "\x1b[31;1m",
            };
            println!("{}{}\x1b[0m", color, message);
        } else {
            println!("{}", message);
        }
    }

    fn log_colored(&self, entry: &LogEntry) {
        let color = match entry.level {
            LogLevel::Debug => "\x1b[90m",
            LogLevel::Info => "\x1b[37m",
            LogLevel::Warning => "\x1b[33m",
            LogLevel::Error => "\x1b[31m",
            LogLevel::Fatal => "\x1b[31;1m",
        };

        let timestamp = entry.timestamp.format("%H:%M:%S");

        if let Some(location) = entry.location_string() {
            println!(
                "\x1b[90m[{}]\x1b[0m {}{}\x1b[0m \x1b[36m{}\x1b[0m",
                timestamp, color, entry.message, location
            );
        } else {
            println!("\x1b[90m[{}]\x1b[0m {}{}", timestamp, color, entry.raw_line);
        }

        std::io::stdout().flush().unwrap();
    }

    fn log_plain(&self, entry: &LogEntry) {
        let timestamp = entry.timestamp.format("%H:%M:%S");

        if let Some(location) = entry.location_string() {
            println!("[{}] {} ({})", timestamp, entry.message, location);
        } else {
            println!("[{}] {}", timestamp, entry.raw_line);
        }

        std::io::stdout().flush().unwrap();
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self::new()
    }
}
