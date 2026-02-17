use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "ignis")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[arg(value_name = "PRESET", help = "Build preset (debug, release, etc.)")]
    pub preset: Option<String>,

    #[arg(
        short = 'C',
        long = "directory",
        value_name = "DIR",
        help = "Source directory"
    )]
    pub source_dir: Option<PathBuf>,

    #[arg(short, long, help = "Skip TUI and use simple logger")]
    pub no_tui: bool,

    #[arg(long, help = "Configuration file path")]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum HistoryType {
    Build,
    Executable,
    All,
}

#[derive(Subcommand, Debug)]
pub enum HistoryCommands {
    #[command(about = "Show history")]
    Show {
        #[arg(short, long, help = "Number of entries to show")]
        count: Option<usize>,
    },

    #[command(about = "Clear history")]
    Clear {
        #[arg(
            long,
            value_enum,
            default_value = "all",
            help = "History type to clear"
        )]
        r#type: HistoryType,
    },
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(about = "Clean build artifacts")]
    Clean {
        #[arg(help = "Build preset")]
        preset: String,
    },

    #[command(about = "List available presets")]
    Presets,

    #[command(about = "Manage history")]
    History {
        #[command(subcommand)]
        command: HistoryCommands,
    },

    #[command(about = "List and manage executables")]
    Exec {
        #[arg(help = "Build preset")]
        preset: String,
    },

    #[command(about = "Initialize a new ignis.toml configuration")]
    Init {
        #[arg(long, help = "Project name")]
        name: Option<String>,
    },
}

impl Cli {
    pub fn source_directory(&self) -> PathBuf {
        self.source_dir
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap())
    }
}
