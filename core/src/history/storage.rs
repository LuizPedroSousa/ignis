use super::{BuildHistoryEntry, ExecutionHistoryEntry};
use anyhow::Context;
use std::fs;
use std::path::Path;

pub fn load_history(path: &Path) -> anyhow::Result<Vec<BuildHistoryEntry>> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read history file: {}", path.display()))?;

    let entries: Vec<BuildHistoryEntry> = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse history file: {}", path.display()))?;

    Ok(entries)
}

pub fn save_history(path: &Path, entries: &[BuildHistoryEntry]) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("Failed to create history directory: {}", parent.display())
        })?;
    }

    let content = serde_json::to_string_pretty(entries)
        .context("Failed to serialize history entries")?;

    fs::write(path, content)
        .with_context(|| format!("Failed to write history file: {}", path.display()))?;

    Ok(())
}

pub fn load_exec_history(path: &Path) -> anyhow::Result<Vec<ExecutionHistoryEntry>> {
    let exec_path = path.with_file_name("exec_history.json");

    if !exec_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&exec_path)
        .with_context(|| format!("Failed to read exec history file: {}", exec_path.display()))?;

    let entries: Vec<ExecutionHistoryEntry> = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse exec history file: {}", exec_path.display()))?;

    Ok(entries)
}

pub fn save_exec_history(path: &Path, entries: &[ExecutionHistoryEntry]) -> anyhow::Result<()> {
    let exec_path = path.with_file_name("exec_history.json");

    if let Some(parent) = exec_path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("Failed to create exec history directory: {}", parent.display())
        })?;
    }

    let content = serde_json::to_string_pretty(entries)
        .context("Failed to serialize exec history entries")?;

    fs::write(&exec_path, content)
        .with_context(|| format!("Failed to write exec history file: {}", exec_path.display()))?;

    Ok(())
}
