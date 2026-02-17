pub mod storage;

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildStepResult {
    pub description: String,
    pub duration: f64,
    pub success: bool,
    pub error_count: usize,
    pub warning_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildHistoryEntry {
    pub timestamp: DateTime<Local>,
    pub preset: String,
    pub duration: f64,
    pub success: bool,
    pub error_count: usize,
    pub warning_count: usize,
    pub steps: Vec<BuildStepResult>,
    pub note: Option<String>,
    pub git_commit: Option<String>,
    pub git_branch: Option<String>,
}

impl BuildHistoryEntry {
    pub fn new(preset: String) -> Self {
        Self {
            timestamp: Local::now(),
            preset,
            duration: 0.0,
            success: false,
            error_count: 0,
            warning_count: 0,
            steps: Vec::new(),
            note: None,
            git_commit: capture_git_commit(),
            git_branch: capture_git_branch(),
        }
    }

    pub fn add_step(&mut self, step: BuildStepResult) {
        self.error_count += step.error_count;
        self.warning_count += step.warning_count;
        self.success = step.success && self.success;
        self.steps.push(step);
    }

    pub fn finalize(&mut self, total_duration: f64) {
        self.duration = total_duration;
        self.success = self.steps.iter().all(|s| s.success);
    }
}

pub struct BuildHistory {
    entries: Vec<BuildHistoryEntry>,
    storage_path: PathBuf,
    max_builds: usize,
}

impl BuildHistory {
    pub fn new(storage_path: PathBuf, max_builds: usize) -> anyhow::Result<Self> {
        let entries = storage::load_history(&storage_path)?;
        Ok(Self {
            entries,
            storage_path,
            max_builds,
        })
    }

    pub fn add_entry(&mut self, entry: BuildHistoryEntry) -> anyhow::Result<()> {
        self.entries.push(entry);

        if self.entries.len() > self.max_builds {
            self.entries.remove(0);
        }

        storage::save_history(&self.storage_path, &self.entries)
    }

    pub fn entries(&self) -> &[BuildHistoryEntry] {
        &self.entries
    }

    pub fn last_entry(&self) -> Option<&BuildHistoryEntry> {
        self.entries.last()
    }

    pub fn clear(&mut self) -> anyhow::Result<()> {
        self.entries.clear();
        storage::save_history(&self.storage_path, &self.entries)
    }
}

fn capture_git_commit() -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
}

fn capture_git_branch() -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|output| {
            if output.status.success() {
                String::from_utf8(output.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionHistoryEntry {
    pub timestamp: DateTime<Local>,
    pub executable_name: String,
    pub executable_path: String,
    pub duration: f64,
    pub exit_code: Option<i32>,
    pub success: bool,
    pub error_count: usize,
    pub warning_count: usize,
    pub metric_count: usize,
    pub log_count: usize,
    #[serde(default)]
    pub failure_reason: Option<String>,
}

impl ExecutionHistoryEntry {
    pub fn new(executable_name: String, executable_path: String) -> Self {
        Self {
            timestamp: Local::now(),
            executable_name,
            executable_path,
            duration: 0.0,
            exit_code: None,
            success: false,
            error_count: 0,
            warning_count: 0,
            metric_count: 0,
            log_count: 0,
            failure_reason: None,
        }
    }
}

pub struct ExecutionHistory {
    entries: Vec<ExecutionHistoryEntry>,
    storage_path: PathBuf,
    max_entries: usize,
}

impl ExecutionHistory {
    pub fn new(storage_path: PathBuf, max_entries: usize) -> anyhow::Result<Self> {
        let entries = storage::load_exec_history(&storage_path)?;
        Ok(Self {
            entries,
            storage_path,
            max_entries,
        })
    }

    pub fn add_entry(&mut self, entry: ExecutionHistoryEntry) -> anyhow::Result<()> {
        self.entries.push(entry);

        if self.entries.len() > self.max_entries {
            self.entries.remove(0);
        }

        storage::save_exec_history(&self.storage_path, &self.entries)
    }

    pub fn entries(&self) -> &[ExecutionHistoryEntry] {
        &self.entries
    }

    pub fn last_entry(&self) -> Option<&ExecutionHistoryEntry> {
        self.entries.last()
    }

    pub fn clear(&mut self) -> anyhow::Result<()> {
        self.entries.clear();
        storage::save_exec_history(&self.storage_path, &self.entries)
    }
}
