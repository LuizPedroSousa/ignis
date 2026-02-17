use super::entry::{LogEntry, LogLevel, LogComponent};
use regex::Regex;

pub trait LogFilter: Send + Sync {
    fn matches(&self, entry: &LogEntry) -> bool;
    fn description(&self) -> String;
}

pub struct LevelFilter {
    min_level: LogLevel,
}

impl LevelFilter {
    pub fn new(min_level: LogLevel) -> Self {
        Self { min_level }
    }
}

impl LogFilter for LevelFilter {
    fn matches(&self, entry: &LogEntry) -> bool {
        entry.level >= self.min_level
    }

    fn description(&self) -> String {
        format!("level >= {}", self.min_level.to_str())
    }
}

pub struct PatternFilter {
    pattern: Regex,
    case_sensitive: bool,
}

impl PatternFilter {
    pub fn new(pattern: &str, case_sensitive: bool) -> Result<Self, regex::Error> {
        let pattern = if case_sensitive {
            Regex::new(pattern)?
        } else {
            Regex::new(&format!("(?i){}", pattern))?
        };
        Ok(Self {
            pattern,
            case_sensitive,
        })
    }
}

impl LogFilter for PatternFilter {
    fn matches(&self, entry: &LogEntry) -> bool {
        self.pattern.is_match(&entry.message) || self.pattern.is_match(&entry.raw_line)
    }

    fn description(&self) -> String {
        format!(
            "pattern: {} ({})",
            self.pattern.as_str(),
            if self.case_sensitive {
                "case-sensitive"
            } else {
                "case-insensitive"
            }
        )
    }
}

pub struct FileFilter {
    file_pattern: Regex,
}

impl FileFilter {
    pub fn new(file_pattern: &str) -> Result<Self, regex::Error> {
        Ok(Self {
            file_pattern: Regex::new(file_pattern)?,
        })
    }
}

impl LogFilter for FileFilter {
    fn matches(&self, entry: &LogEntry) -> bool {
        entry
            .file_path
            .as_ref()
            .map(|path| self.file_pattern.is_match(path))
            .unwrap_or(false)
    }

    fn description(&self) -> String {
        format!("file matches: {}", self.file_pattern.as_str())
    }
}

pub struct ComponentFilter {
    component: LogComponent,
}

impl ComponentFilter {
    pub fn new(component: LogComponent) -> Self {
        Self { component }
    }
}

impl LogFilter for ComponentFilter {
    fn matches(&self, entry: &LogEntry) -> bool {
        entry.component == self.component
    }

    fn description(&self) -> String {
        format!("component: {:?}", self.component)
    }
}

pub struct CompositeFilter {
    filters: Vec<Box<dyn LogFilter>>,
    mode: FilterMode,
}

pub enum FilterMode {
    And,
    Or,
}

impl CompositeFilter {
    pub fn new(filters: Vec<Box<dyn LogFilter>>, mode: FilterMode) -> Self {
        Self { filters, mode }
    }

    pub fn and(filters: Vec<Box<dyn LogFilter>>) -> Self {
        Self::new(filters, FilterMode::And)
    }

    pub fn or(filters: Vec<Box<dyn LogFilter>>) -> Self {
        Self::new(filters, FilterMode::Or)
    }
}

impl LogFilter for CompositeFilter {
    fn matches(&self, entry: &LogEntry) -> bool {
        match self.mode {
            FilterMode::And => self.filters.iter().all(|f| f.matches(entry)),
            FilterMode::Or => self.filters.iter().any(|f| f.matches(entry)),
        }
    }

    fn description(&self) -> String {
        let mode = match self.mode {
            FilterMode::And => "AND",
            FilterMode::Or => "OR",
        };
        let descriptions: Vec<_> = self.filters.iter().map(|f| f.description()).collect();
        format!("({})", descriptions.join(&format!(" {} ", mode)))
    }
}
