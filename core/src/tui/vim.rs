use crate::parser::filters::{LevelFilter, LogFilter, PatternFilter};
use crate::parser::entry::LogLevel;
use super::keybinding_manager::PendingSequence;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Command,
    Search,
}

#[derive(Debug)]
pub struct VimCommandMode {
    pub mode: InputMode,
    pub input_buffer: String,
    pub search_pattern: Option<String>,
    pub search_index: usize,
    pub pending_sequence: Option<PendingSequence>,
    pub count_buffer: String,
}

impl VimCommandMode {
    pub fn new() -> Self {
        Self {
            mode: InputMode::Normal,
            input_buffer: String::new(),
            search_pattern: None,
            search_index: 0,
            pending_sequence: None,
            count_buffer: String::new(),
        }
    }

    pub fn enter_command_mode(&mut self) {
        self.mode = InputMode::Command;
        self.input_buffer.clear();
    }

    pub fn enter_search_mode(&mut self) {
        self.mode = InputMode::Search;
        self.input_buffer.clear();
    }

    pub fn exit_to_normal(&mut self) {
        self.mode = InputMode::Normal;
        self.input_buffer.clear();
    }

    pub fn push_char(&mut self, c: char) {
        self.input_buffer.push(c);
    }

    pub fn pop_char(&mut self) {
        self.input_buffer.pop();
    }

    pub fn execute_command(&mut self) -> Option<CommandResult> {
        let cmd = self.input_buffer.trim();

        let result = if cmd == "q" || cmd == "quit" {
            Some(CommandResult::Quit)
        } else if cmd == "w" || cmd.starts_with("w ") {
            let file = cmd.strip_prefix("w ").map(|s| s.trim().to_string());
            Some(CommandResult::WriteLogs(file))
        } else if cmd == "fl" || cmd.starts_with("filter level=") {
            if let Some(level_str) = cmd.strip_prefix("filter level=") {
                let filter = match level_str.to_uppercase().as_str() {
                    "ERROR" => Some(Box::new(LevelFilter::new(LogLevel::Error)) as Box<dyn LogFilter>),
                    "WARNING" => Some(Box::new(LevelFilter::new(LogLevel::Warning)) as Box<dyn LogFilter>),
                    "INFO" => Some(Box::new(LevelFilter::new(LogLevel::Info)) as Box<dyn LogFilter>),
                    "DEBUG" => Some(Box::new(LevelFilter::new(LogLevel::Debug)) as Box<dyn LogFilter>),
                    _ => None,
                };
                filter.map(CommandResult::ApplyFilter)
            } else {
                None
            }
        } else if cmd == "nofilter" || cmd == "nf" {
            Some(CommandResult::ClearFilter)
        } else if let Ok(line_number) = cmd.parse::<usize>() {
            Some(CommandResult::GotoLine(line_number))
        } else {
            None
        };

        self.exit_to_normal();
        result
    }

    pub fn execute_search(&mut self) -> Option<CommandResult> {
        let pattern = self.input_buffer.trim().to_string();

        if pattern.is_empty() {
            self.exit_to_normal();
            return None;
        }

        self.search_pattern = Some(pattern.clone());
        self.search_index = 0;

        let filter = PatternFilter::new(&pattern, false).ok()?;
        self.exit_to_normal();
        Some(CommandResult::Search(pattern, Box::new(filter)))
    }

    pub fn next_search(&mut self) {
        self.search_index += 1;
    }

    pub fn prev_search(&mut self) {
        if self.search_index > 0 {
            self.search_index -= 1;
        }
    }

    pub fn start_sequence(&mut self, key: super::keybinding_manager::KeyPress) {
        self.pending_sequence = Some(PendingSequence::new(key));
    }

    pub fn add_to_sequence(&mut self, key: super::keybinding_manager::KeyPress) {
        if let Some(seq) = &mut self.pending_sequence {
            seq.add_key(key);
        }
    }

    pub fn clear_sequence(&mut self) {
        self.pending_sequence = None;
    }

    pub fn is_sequence_timeout(&self, timeout_ms: u64) -> bool {
        if let Some(seq) = &self.pending_sequence {
            seq.is_timeout(timeout_ms)
        } else {
            false
        }
    }

    pub fn get_sequence_display(&self) -> Option<String> {
        self.pending_sequence.as_ref().map(|s| s.get_display_string())
    }

    pub fn push_count_digit(&mut self, digit: char) {
        if digit.is_ascii_digit() {
            self.count_buffer.push(digit);
        }
    }

    pub fn get_count(&self) -> usize {
        self.count_buffer.parse::<usize>().unwrap_or(1)
    }

    pub fn clear_count(&mut self) {
        self.count_buffer.clear();
    }

    pub fn has_count(&self) -> bool {
        !self.count_buffer.is_empty()
    }

    pub fn get_count_display(&self) -> Option<String> {
        if self.has_count() {
            Some(self.count_buffer.clone())
        } else {
            None
        }
    }
}

impl Default for VimCommandMode {
    fn default() -> Self {
        Self::new()
    }
}

pub enum CommandResult {
    Quit,
    WriteLogs(Option<String>),
    ApplyFilter(Box<dyn LogFilter>),
    ClearFilter,
    Search(String, Box<dyn LogFilter>),
    GotoLine(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goto_line_command() {
        let mut vim_mode = VimCommandMode::new();
        vim_mode.mode = InputMode::Command;
        vim_mode.input_buffer = "123".to_string();

        let result = vim_mode.execute_command();
        assert!(matches!(result, Some(CommandResult::GotoLine(123))));
    }

    #[test]
    fn test_quit_command() {
        let mut vim_mode = VimCommandMode::new();
        vim_mode.mode = InputMode::Command;
        vim_mode.input_buffer = "q".to_string();

        let result = vim_mode.execute_command();
        assert!(matches!(result, Some(CommandResult::Quit)));
    }

    #[test]
    fn test_invalid_command() {
        let mut vim_mode = VimCommandMode::new();
        vim_mode.mode = InputMode::Command;
        vim_mode.input_buffer = "invalid".to_string();

        let result = vim_mode.execute_command();
        assert!(result.is_none());
    }

    #[test]
    fn test_count_buffer() {
        let mut vim_mode = VimCommandMode::new();

        assert!(!vim_mode.has_count());
        assert_eq!(vim_mode.get_count(), 1);

        vim_mode.push_count_digit('4');
        assert!(vim_mode.has_count());
        assert_eq!(vim_mode.get_count(), 4);

        vim_mode.push_count_digit('2');
        assert_eq!(vim_mode.get_count(), 42);

        vim_mode.clear_count();
        assert!(!vim_mode.has_count());
        assert_eq!(vim_mode.get_count(), 1);
    }

    #[test]
    fn test_count_display() {
        let mut vim_mode = VimCommandMode::new();

        assert_eq!(vim_mode.get_count_display(), None);

        vim_mode.push_count_digit('1');
        vim_mode.push_count_digit('0');
        assert_eq!(vim_mode.get_count_display(), Some("10".to_string()));
    }
}
