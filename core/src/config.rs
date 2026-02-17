use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::target::{Target, TargetKind};
use crate::Cli;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub history: HistoryConfig,
    #[serde(default)]
    pub logs: LogsConfig,
    #[serde(default)]
    pub editor: EditorConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub keybindings: KeybindingsConfig,
    #[serde(default)]
    pub build: BuildConfig,
    #[serde(default)]
    pub stages: StagesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryConfig {
    #[serde(default = "default_max_builds")]
    pub max_builds: usize,
    #[serde(default = "default_storage_path")]
    pub storage_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogsConfig {
    #[serde(default)]
    pub auto_save: bool,
    #[serde(default = "default_save_directory")]
    pub save_directory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    #[serde(default = "default_editor_command")]
    pub command: String,
    #[serde(default = "default_true")]
    pub vscode_integration: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    #[serde(default = "default_max_log_lines")]
    pub max_log_lines: usize,
    #[serde(default = "default_true")]
    pub show_timestamps: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindingsConfig {
    #[serde(default = "default_leader_key")]
    pub leader_key: String,
    #[serde(default = "default_sequence_timeout")]
    pub sequence_timeout_ms: u64,
    #[serde(default)]
    pub leader_bindings: HashMap<String, String>,
    #[serde(default)]
    pub vim_sequences: HashMap<String, String>,
    #[serde(default = "default_enable_leader")]
    pub enable_leader: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildConfig {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagesConfig {
    #[serde(default = "default_enabled_stages")]
    pub enabled_stages: Vec<String>,
    #[serde(default = "default_auto_build_on_start")]
    pub auto_build_on_start: bool,
    #[serde(default)]
    pub stage_dependencies: HashMap<String, Vec<String>>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            history: HistoryConfig::default(),
            logs: LogsConfig::default(),
            editor: EditorConfig::default(),
            display: DisplayConfig::default(),
            keybindings: KeybindingsConfig::default(),
            build: BuildConfig::default(),
            stages: StagesConfig::default(),
        }
    }
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self {
            max_builds: default_max_builds(),
            storage_path: default_storage_path(),
        }
    }
}

impl Default for LogsConfig {
    fn default() -> Self {
        Self {
            auto_save: false,
            save_directory: default_save_directory(),
        }
    }
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            command: default_editor_command(),
            vscode_integration: true,
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            max_log_lines: default_max_log_lines(),
            show_timestamps: true,
            theme: default_theme(),
        }
    }
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            leader_key: default_leader_key(),
            sequence_timeout_ms: default_sequence_timeout(),
            leader_bindings: HashMap::new(),
            vim_sequences: HashMap::new(),
            enable_leader: default_enable_leader(),
        }
    }
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            kind: None,
            name: None,
        }
    }
}

impl Default for StagesConfig {
    fn default() -> Self {
        Self {
            enabled_stages: default_enabled_stages(),
            auto_build_on_start: default_auto_build_on_start(),
            stage_dependencies: HashMap::new(),
        }
    }
}

impl BuildConfig {
    pub fn target_kind(&self) -> TargetKind {
        match self.kind.as_deref() {
            Some("installer") => TargetKind::Installer,
            Some("root") => TargetKind::Root,
            Some("executable") => TargetKind::Executable,
            Some(v) => panic!("Unrecognized build kind: {}", v),
            _ => panic!("Missing build kind: \"installer\"|\"root\"|\"executable\""),
        }
    }
}

fn default_max_builds() -> usize {
    50
}

fn default_storage_path() -> String {
    "~/.astralix/build_history.json".to_string()
}

fn default_save_directory() -> String {
    "~/.cache/astralix/logs".to_string()
}

fn default_editor_command() -> String {
    "${EDITOR}".to_string()
}

fn default_max_log_lines() -> usize {
    10000
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_true() -> bool {
    true
}

fn default_leader_key() -> String {
    "Space".to_string()
}

fn default_sequence_timeout() -> u64 {
    1000
}

fn default_enable_leader() -> bool {
    true
}

fn default_enabled_stages() -> Vec<String> {
    vec![
        "PreValidation".to_string(),
        "Configure".to_string(),
        "Build".to_string(),
        "Install".to_string(),
    ]
}

fn default_auto_build_on_start() -> bool {
    false
}

impl Config {
    pub fn load_from_cli(cli: &Cli) -> Result<(Target, Vec<Target>), anyhow::Error> {
        let source_dir = cli.source_directory();
        let root = Config::find_config(source_dir.clone());
        let mut targets = Vec::new();

        if root.config.build.target_kind() != TargetKind::Root {
            let root_as_target = Target {
                path: root.path.clone(),
                kind: root.config.build.target_kind(),
                config: root.config.clone(),
            };
            targets.push(root_as_target);
        }

        targets.extend(Config::find_targets_configs(&source_dir));

        targets.sort_by_key(|target| match target.kind {
            TargetKind::Executable => 0,
            TargetKind::Installer => 1,
            TargetKind::Root => 2,
        });

        Ok((root, targets))
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let expanded = shellexpand::tilde(path.as_ref().to_str().unwrap());
        let path = Path::new(expanded.as_ref());

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

        Ok(config)
    }

    pub fn merge_with(mut self, other: Config) -> Self {
        if !other.keybindings.leader_bindings.is_empty() {
            self.keybindings
                .leader_bindings
                .extend(other.keybindings.leader_bindings);
        }
        if !other.keybindings.vim_sequences.is_empty() {
            self.keybindings
                .vim_sequences
                .extend(other.keybindings.vim_sequences);
        }
        self
    }

    pub fn global_path() -> PathBuf {
        let expanded = shellexpand::tilde("~/.config/astralix/ignis.toml");
        PathBuf::from(expanded.as_ref())
    }

    pub fn expand_path(path: &str) -> PathBuf {
        let expanded = shellexpand::tilde(path);
        PathBuf::from(expanded.as_ref())
    }

    pub fn storage_path(&self) -> PathBuf {
        Self::expand_path(&self.history.storage_path)
    }

    pub fn log_directory(&self) -> PathBuf {
        Self::expand_path(&self.logs.save_directory)
    }

    pub fn find_targets_configs(root_dir: &Path) -> Vec<Target> {
        let mut targets: Vec<Target> = Vec::new();

        fn search_recursive(dir: &Path, targets: &mut Vec<Target>, root_config: &Path) {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let config_path = path.join("ignis.toml");

                        if config_path.exists() && config_path != root_config {
                            let config = Config::load_from_file(&config_path).unwrap_or_default();
                            targets.push(Target {
                                path: path.clone(),
                                kind: config.build.target_kind(),
                                config,
                            });
                        }

                        search_recursive(&path, targets, root_config);
                    }
                }
            }
        }

        let root_config = root_dir.join("ignis.toml");
        search_recursive(root_dir, &mut targets, &root_config);
        targets
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self).context("Failed to serialize config to TOML")?;

        std::fs::write(path.as_ref(), content)
            .with_context(|| format!("Failed to write config file: {}", path.as_ref().display()))?;

        Ok(())
    }

    fn find_config(root_path: std::path::PathBuf) -> Target {
        let root_config =
            Config::load_from_file(root_path.join("ignis.toml")).unwrap_or_else(|_| {
                panic!(
                    "No ignis.toml found in {} or any parent directory.\n\
                Run 'ignis init' to create a new configuration.",
                    root_path.display()
                )
            });

        let global_config = Config::load_from_file(Config::global_path()).unwrap_or_default();

        Target {
            path: root_path,
            kind: TargetKind::Root,
            config: root_config.merge_with(global_config),
        }
    }
}
