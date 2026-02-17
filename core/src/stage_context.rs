use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

use crate::{
    builder::{BuildStep, Builder},
    executor::StepUpdate,
    parser::entry::LogEntry,
    stage::Stage,
    ExecutableInfo,
};

#[derive(Debug, Clone)]
pub enum StageResult {
    Success {
        duration: f64,
        steps_executed: usize,
    },
    Failed {
        error: String,
        duration: f64,
        steps_executed: usize,
    },
    Skipped {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

pub struct StageContext {
    builder: Builder,
    log_tx: mpsc::UnboundedSender<LogEntry>,
    step_tx: mpsc::UnboundedSender<StepUpdate>,
    executable_info: Option<ExecutableInfo>,
}

impl StageContext {
    pub fn new(
        builder: Builder,
        log_tx: mpsc::UnboundedSender<LogEntry>,
        step_tx: mpsc::UnboundedSender<StepUpdate>,
    ) -> Self {
        Self {
            builder,
            log_tx,
            step_tx,
            executable_info: None,
        }
    }

    pub fn with_executable(mut self, exec_info: ExecutableInfo) -> Self {
        self.executable_info = Some(exec_info);
        self
    }

    pub fn builder(&self) -> &Builder {
        &self.builder
    }

    pub fn log_tx(&self) -> mpsc::UnboundedSender<LogEntry> {
        self.log_tx.clone()
    }

    pub fn step_tx(&self) -> mpsc::UnboundedSender<StepUpdate> {
        self.step_tx.clone()
    }

    pub fn executable_info(&self) -> Option<&ExecutableInfo> {
        self.executable_info.as_ref()
    }

    pub fn generate_steps_for_stage(&self, stage: Stage) -> Vec<BuildStep> {
        match stage {
            Stage::PreValidation => self.generate_prevalidation_steps(),
            Stage::Configure => self.generate_configure_steps(),
            Stage::Build => self.generate_build_steps(),
            Stage::Install => self.generate_install_steps(),
            Stage::Clean => self.generate_clean_steps(),
            Stage::PostBuild => self.generate_postbuild_steps(),
            Stage::Test => self.generate_test_steps(),
            Stage::Exec => {
                vec![]
            }
        }
    }

    fn generate_prevalidation_steps(&self) -> Vec<BuildStep> {
        vec![]
    }

    fn generate_configure_steps(&self) -> Vec<BuildStep> {
        let mut steps = Vec::new();

        for target in self.builder.targets() {
            let (build_dir, _install_dir) = self
                .builder
                .get_dirs(target.path.clone(), self.builder.preset());

            let build_dir_str = build_dir.display().to_string();
            let source_dir = target.path.display().to_string();

            let cmake = self.builder.cmake();
            steps.push(cmake.configure_step("CMake", source_dir, build_dir_str));
        }

        steps
    }

    fn generate_build_steps(&self) -> Vec<BuildStep> {
        let mut steps = Vec::new();

        for target in self.builder.targets() {
            let (build_dir, _install_dir) = self
                .builder
                .get_dirs(target.path.clone(), self.builder.preset());

            let build_dir_str = build_dir.display().to_string();
            let target_name = target.config.build.name.as_deref().unwrap_or("project");

            let cmake = self.builder.cmake();
            steps.push(cmake.build_step(target_name, build_dir_str));
        }

        steps
    }

    fn generate_install_steps(&self) -> Vec<BuildStep> {
        let mut steps = Vec::new();

        for target in self.builder.targets() {
            if target.kind == crate::target::TargetKind::Installer {
                let (build_dir, install_dir) = self
                    .builder
                    .get_dirs(target.path.clone(), self.builder.preset());

                let build_dir_str = build_dir.display().to_string();
                let install_dir_str = install_dir.display().to_string();

                let cmake = self.builder.cmake();
                steps.push(cmake.install_step("artifacts", build_dir_str, install_dir_str));
            }
        }

        steps
    }

    fn generate_clean_steps(&self) -> Vec<BuildStep> {
        self.builder.generate_clean_all()
    }

    fn generate_postbuild_steps(&self) -> Vec<BuildStep> {
        vec![]
    }

    fn generate_test_steps(&self) -> Vec<BuildStep> {
        vec![]
    }
}

pub type SharedStageContext = Arc<Mutex<StageContext>>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{target::Target, Config};

    fn create_test_builder() -> Builder {
        let root_config = Config::default();
        let root = Target {
            path: std::path::PathBuf::from("/tmp/test"),
            kind: crate::target::TargetKind::Root,
            config: root_config.clone(),
        };
        Builder::new(root, "debug".to_string())
    }

    #[test]
    fn test_stage_result_success() {
        let result = StageResult::Success {
            duration: 1.5,
            steps_executed: 3,
        };

        match result {
            StageResult::Success {
                duration,
                steps_executed,
            } => {
                assert_eq!(duration, 1.5);
                assert_eq!(steps_executed, 3);
            }
            _ => panic!("Expected Success"),
        }
    }

    #[test]
    fn test_stage_status() {
        assert_eq!(StageStatus::Pending, StageStatus::Pending);
        assert_ne!(StageStatus::Running, StageStatus::Completed);
    }

    #[test]
    fn test_stage_context_creation() {
        let builder = create_test_builder();
        let (log_tx, _log_rx) = mpsc::unbounded_channel();
        let (step_tx, _step_rx) = mpsc::unbounded_channel();

        let context = StageContext::new(builder, log_tx, step_tx);
        assert!(context.executable_info().is_none());
    }

    #[test]
    fn test_generate_steps_for_stage() {
        let builder = create_test_builder();
        let (log_tx, _log_rx) = mpsc::unbounded_channel();
        let (step_tx, _step_rx) = mpsc::unbounded_channel();

        let context = StageContext::new(builder, log_tx, step_tx);

        let steps = context.generate_steps_for_stage(Stage::PreValidation);
        assert_eq!(steps.len(), 0);

        let steps = context.generate_steps_for_stage(Stage::Exec);
        assert_eq!(steps.len(), 0);
    }
}
