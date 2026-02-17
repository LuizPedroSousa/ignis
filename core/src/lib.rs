pub mod builder;
pub mod cli;
pub mod command;
pub mod config;
pub mod dependency_graph;
pub mod editor;
pub mod executor;
pub mod history;
pub mod logger;
pub mod monitor;
pub mod parser;
pub mod stage;
pub mod stage_context;
pub mod stage_runner;
pub mod target;
pub mod tui;

pub use builder::{Builder, ExecutableInfo};
pub use cli::{Cli, HistoryCommands, HistoryType};
pub use config::Config;
pub use dependency_graph::{GraphError, StageDependencyGraph};
pub use executor::{
    execute_program, execute_step, ExecutionResult, MetricHistory, MetricType, MetricVisualization,
    RuntimeMetric, StepUpdate,
};
pub use history::{
    BuildHistory, BuildHistoryEntry, BuildStepResult, ExecutionHistory, ExecutionHistoryEntry,
};
pub use monitor::{ResourceMonitor, ResourceStats};
pub use stage::{Stage, StageMetadata};
pub use stage_context::{SharedStageContext, StageContext, StageResult, StageStatus};
pub use stage_runner::{BuildContext, ExecRunner, StageRunner};
pub use tui::{BuildAction, ExecAction};

pub mod runner {
    pub use crate::stage_runner::BuildContext;
}
