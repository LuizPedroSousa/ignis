use anyhow::Context;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::{
    dependency_graph::StageDependencyGraph,
    executor,
    parser::CompilerOutputParser,
    stage::Stage,
    stage_context::{StageContext, StageResult, StageStatus},
    tui::App,
    BuildHistory, ResourceMonitor,
};
use crate::{tui, BuildAction, Builder, ExecAction, ExecutableInfo};

#[derive(Clone)]
pub struct BuildContext {
    builder: Builder,
}

impl BuildContext {
    pub fn new(builder: Builder) -> Self {
        Self { builder }
    }

    pub fn builder(&self) -> &Builder {
        &self.builder
    }
}

pub struct StageRunner {
    ctx: BuildContext,
    results: Arc<Mutex<HashMap<Stage, StageResult>>>,
    statuses: Arc<Mutex<HashMap<Stage, StageStatus>>>,
}

pub struct ExecContext {
    app: Option<App>,
    info: Option<ExecutableInfo>,
}

pub struct ExecRunner {
    ctx: ExecContext,
}

impl StageRunner {
    pub fn new(ctx: BuildContext) -> Self {
        Self {
            ctx,
            results: Arc::new(Mutex::new(HashMap::new())),
            statuses: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn idle(
        &self,
    ) -> Result<
        (
            App,
            mpsc::UnboundedSender<crate::parser::entry::LogEntry>,
            mpsc::UnboundedSender<crate::executor::StepUpdate>,
        ),
        anyhow::Error,
    > {
        let (log_tx, log_rx) = mpsc::unbounded_channel();
        let (step_tx, step_rx) = mpsc::unbounded_channel();

        let root = self.ctx.builder().root();
        let storage_path = root.config.storage_path();
        let max_builds = root.config.history.max_builds;

        let build_history =
            BuildHistory::new(storage_path, max_builds).context("Failed to load build history")?;

        let resource_monitor = ResourceMonitor::new();
        resource_monitor.clone().start_monitoring();

        let app = App::new(
            build_history,
            log_rx,
            step_rx,
            resource_monitor,
            self.ctx.builder().clone(),
        );

        Ok((app, log_tx, step_tx))
    }

    pub async fn execute_stage(
        &self,
        stage: Stage,
        context: &StageContext,
    ) -> Result<StageResult, anyhow::Error> {
        let start = Instant::now();

        {
            let mut statuses = self.statuses.lock().unwrap();
            statuses.insert(stage, StageStatus::Running);
        }

        if stage == Stage::Exec {
            if let Some(exec_info) = context.executable_info() {
                let result = executor::execute_program(
                    exec_info.clone(),
                    context.log_tx(),
                    context.step_tx(),
                )
                .await?;

                let stage_result = if result.success {
                    StageResult::Success {
                        duration: result.duration,
                        steps_executed: 1,
                    }
                } else {
                    StageResult::Failed {
                        error: result
                            .failure_reason
                            .unwrap_or_else(|| "Execution failed".to_string()),
                        duration: result.duration,
                        steps_executed: 1,
                    }
                };

                {
                    let mut results = self.results.lock().unwrap();
                    results.insert(stage, stage_result.clone());
                    let mut statuses = self.statuses.lock().unwrap();
                    statuses.insert(
                        stage,
                        if result.success {
                            StageStatus::Completed
                        } else {
                            StageStatus::Failed
                        },
                    );
                }

                return Ok(stage_result);
            } else {
                let error = "Exec stage requires executable info".to_string();
                let stage_result = StageResult::Failed {
                    error: error.clone(),
                    duration: 0.0,
                    steps_executed: 0,
                };

                {
                    let mut results = self.results.lock().unwrap();
                    results.insert(stage, stage_result.clone());
                    let mut statuses = self.statuses.lock().unwrap();
                    statuses.insert(stage, StageStatus::Failed);
                }

                return Ok(stage_result);
            }
        }

        let steps = context.generate_steps_for_stage(stage);

        if steps.is_empty() {
            let stage_result = StageResult::Skipped {
                reason: "No steps to execute".to_string(),
            };

            {
                let mut results = self.results.lock().unwrap();
                results.insert(stage, stage_result.clone());
                let mut statuses = self.statuses.lock().unwrap();
                statuses.insert(stage, StageStatus::Skipped);
            }

            return Ok(stage_result);
        }

        let mut parser = CompilerOutputParser::new();
        let log_tx = context.log_tx();
        let step_tx = context.step_tx();

        let execution_results = executor::execute_steps(
            steps,
            move |line| {
                let entry = parser.parse_line(&line);
                let _ = log_tx.send(entry);
            },
            step_tx,
        )
        .await?;

        let duration = start.elapsed().as_secs_f64();
        let steps_executed = execution_results.len();
        let success = execution_results.iter().all(|r| r.success);

        let stage_result = if success {
            StageResult::Success {
                duration,
                steps_executed,
            }
        } else {
            let error = execution_results
                .iter()
                .find(|r| !r.success)
                .and_then(|r| r.failure_reason.clone())
                .unwrap_or_else(|| "Stage execution failed".to_string());

            StageResult::Failed {
                error,
                duration,
                steps_executed,
            }
        };

        {
            let mut results = self.results.lock().unwrap();
            results.insert(stage, stage_result.clone());
            let mut statuses = self.statuses.lock().unwrap();
            statuses.insert(
                stage,
                if success {
                    StageStatus::Completed
                } else {
                    StageStatus::Failed
                },
            );
        }

        Ok(stage_result)
    }

    pub async fn execute_stages_concurrent(
        &self,
        stages: Vec<Stage>,
        context: Arc<Mutex<StageContext>>,
    ) -> Result<HashMap<Stage, StageResult>, anyhow::Error> {
        let mut handles: Vec<JoinHandle<Result<(Stage, StageResult), anyhow::Error>>> = Vec::new();

        for stage in stages {
            let context_clone = Arc::clone(&context);
            let runner = Self {
                ctx: self.ctx.clone(),
                results: Arc::clone(&self.results),
                statuses: Arc::clone(&self.statuses),
            };

            let handle = tokio::spawn(async move {
                let new_ctx = {
                    let ctx = context_clone.lock().unwrap();
                    StageContext::new(ctx.builder().clone(), ctx.log_tx(), ctx.step_tx())
                };

                let result = runner.execute_stage(stage, &new_ctx).await?;
                Ok((stage, result))
            });

            handles.push(handle);
        }

        let mut results = HashMap::new();
        for handle in handles {
            let (stage, result) = handle.await??;
            results.insert(stage, result);
        }

        Ok(results)
    }

    pub async fn execute_with_dependencies(
        &self,
        stages: Vec<Stage>,
        context: StageContext,
    ) -> Result<HashMap<Stage, StageResult>, anyhow::Error> {
        let graph = StageDependencyGraph::from_stages(stages);

        let layers = graph
            .topological_sort()
            .context("Failed to resolve stage dependencies")?;

        let context = Arc::new(Mutex::new(context));
        let mut all_results = HashMap::new();

        for layer in layers {
            let previous_failures = {
                let results = self.results.lock().unwrap();
                results
                    .values()
                    .any(|r| matches!(r, StageResult::Failed { .. }))
            };

            if previous_failures {
                for stage in layer {
                    let skip_result = StageResult::Skipped {
                        reason: "Previous stage failed".to_string(),
                    };
                    let mut results = self.results.lock().unwrap();
                    results.insert(stage, skip_result.clone());
                    let mut statuses = self.statuses.lock().unwrap();
                    statuses.insert(stage, StageStatus::Skipped);
                    all_results.insert(stage, skip_result);
                }
                continue;
            }

            let can_run_concurrent = layer.iter().all(|s| s.metadata().can_run_concurrent);

            if can_run_concurrent && layer.len() > 1 {
                let layer_results = self
                    .execute_stages_concurrent(layer, Arc::clone(&context))
                    .await?;
                all_results.extend(layer_results);
            } else {
                for stage in layer {
                    let new_ctx = {
                        let ctx = context.lock().unwrap();
                        StageContext::new(ctx.builder().clone(), ctx.log_tx(), ctx.step_tx())
                    };

                    let result = self.execute_stage(stage, &new_ctx).await?;
                    all_results.insert(stage, result);

                    if matches!(all_results.get(&stage), Some(StageResult::Failed { .. })) {
                        break;
                    }
                }
            }
        }

        Ok(all_results)
    }

    pub fn get_status(&self, stage: Stage) -> Option<StageStatus> {
        let statuses = self.statuses.lock().unwrap();
        statuses.get(&stage).cloned()
    }

    pub fn get_result(&self, stage: Stage) -> Option<StageResult> {
        let results = self.results.lock().unwrap();
        results.get(&stage).cloned()
    }

    pub fn clear(&self) {
        let mut results = self.results.lock().unwrap();
        results.clear();
        let mut statuses = self.statuses.lock().unwrap();
        statuses.clear();
    }
}

impl ExecRunner {
    pub fn new() -> Self {
        Self {
            ctx: ExecContext::new(),
        }
    }

    pub async fn run(
        &mut self,
        builder: &Builder,
        selected_exec: ExecutableInfo,
    ) -> Result<(), anyhow::Error> {
        let mut resume_mode = self.ctx.should_resume(&selected_exec);

        loop {
            let app_to_use = if resume_mode {
                self.ctx.app.take()
            } else {
                None
            };

            let (returned_app, exec_action, build_action) = self
                .handle_mode(builder, selected_exec.clone(), resume_mode, app_to_use)
                .await?;

            if let Some(build_action) = build_action {
                self.ctx.store(returned_app, selected_exec.clone());

                if self
                    .handle_action(builder, &selected_exec, build_action)
                    .await?
                {
                    continue;
                } else {
                    break;
                }
            }

            match exec_action {
                tui::ExecAction::QuitToBuild => {
                    self.ctx.store(returned_app, selected_exec.clone());
                    break;
                }
                tui::ExecAction::Restart => {
                    self.ctx.clear();
                    resume_mode = false;
                }
                tui::ExecAction::Kill => {
                    self.ctx.store(returned_app, selected_exec.clone());
                    break;
                }
            }
        }

        Ok(())
    }

    async fn handle_mode(
        &self,
        builder: &Builder,
        exec_info: ExecutableInfo,
        resume_mode: bool,
        cached_app: Option<App>,
    ) -> Result<(Option<App>, ExecAction, Option<BuildAction>), anyhow::Error> {
        use ResourceMonitor;

        if resume_mode && cached_app.is_some() {
            let mut app = cached_app.unwrap();
            app.run().await?;
            let action = app.get_exec_action().unwrap_or(ExecAction::QuitToBuild);
            let build_action = app.get_build_action();

            match action {
                ExecAction::QuitToBuild => Ok((Some(app), action, build_action)),
                _ => Ok((None, action, build_action)),
            }
        } else {
            let (log_tx, log_rx) = mpsc::unbounded_channel();
            let (step_tx, step_rx) = mpsc::unbounded_channel();

            let resource_monitor = ResourceMonitor::new();
            let _monitor_handle = resource_monitor.clone().start_monitoring();

            let mut app = App::new_exec_mode(
                log_rx,
                step_rx,
                resource_monitor,
                exec_info.clone(),
                builder.clone(),
            );

            let exec_handle =
                tokio::spawn(
                    async move { executor::execute_program(exec_info, log_tx, step_tx).await },
                );

            let app_handle = tokio::spawn(async move {
                app.run().await?;
                Ok::<_, anyhow::Error>(app)
            });

            let _exec_result = exec_handle.await??;
            let mut app = app_handle.await??;

            app.finalize_exec()?;

            let action = app.get_exec_action().unwrap_or(ExecAction::QuitToBuild);
            let build_action = app.get_build_action();

            match action {
                ExecAction::QuitToBuild => Ok((Some(app), action, build_action)),
                _ => Ok((None, action, build_action)),
            }
        }
    }

    // pub async fn handle_mode_menu(
    //     &self,
    //     cli: &Cli,
    //     preset: &str,
    //     config: &Config,
    // ) -> Result<(), anyhow::Error> {
    //     let source_dir = cli.source_directory();
    //
    //     let (log_tx, log_rx) = mpsc::unbounded_channel();
    //     let (step_tx, step_rx) = mpsc::unbounded_channel();
    //
    //     let storage_path = config.storage_path();
    //     let max_builds = config.history.max_builds;
    //
    //     let build_history =
    //         BuildHistory::new(storage_path, max_builds).context("Failed to load build history")?;
    //
    //     let resource_monitor = ResourceMonitor::new();
    //     let _monitor_handle = resource_monitor.clone().start_monitoring();
    //
    //     let mut app = App::new(
    //         config.clone(),
    //         build_history,
    //         log_rx,
    //         step_rx,
    //         resource_monitor,
    //         source_dir.clone(),
    //         preset.to_string(),
    //     );
    //
    //     drop(log_tx);
    //     drop(step_tx);
    //
    //     app.run().await?;
    //
    //     if let Some(selected_exec) = app.get_selected_executable() {
    //         let builder = Builder::new(source_dir.clone(), preset.to_string())
    //             .with_targets(config.build.targets.clone());
    //
    //         loop {
    //             let (_app, action, build_action) = self
    //                 .handle_mode(selected_exec.clone(), config, false, None)
    //                 .await?;
    //
    //             if let Some(build_action) = build_action {
    //                 match build_action {
    //                     BuildAction::Rebuild => {
    //                         println!("Rebuilding {}...", selected_exec.name);
    //                         let target_steps = builder.generate_build_target_steps(&selected_exec);
    //
    //                         for (description, command) in target_steps {
    //                             let result = executor::execute_step(
    //                                 description,
    //                                 command,
    //                                 |line| println!("{}", line),
    //                                 None,
    //                             )
    //                             .await?;
    //
    //                             if !result.success {
    //                                 eprintln!("Build failed for target: {}", selected_exec.name);
    //                                 break;
    //                             }
    //                         }
    //                         println!("Build completed.");
    //                     }
    //                     BuildAction::Clean => {
    //                         println!("Cleaning {}...", selected_exec.name);
    //                         let clean_cmd = builder.generate_clean_target_command(&selected_exec);
    //                         let _ = executor::execute_step(
    //                             format!("Cleaning {}", selected_exec.name),
    //                             clean_cmd,
    //                             |line| println!("{}", line),
    //                             None,
    //                         )
    //                         .await?;
    //                         println!("Clean completed.");
    //
    //                         println!("Rebuilding {}...", selected_exec.name);
    //                         let target_steps = builder.generate_build_target_steps(&selected_exec);
    //
    //                         for (description, command) in target_steps {
    //                             let result = executor::execute_step(
    //                                 description,
    //                                 command,
    //                                 |line| println!("{}", line),
    //                                 None,
    //                             )
    //                             .await?;
    //
    //                             if !result.success {
    //                                 eprintln!("Build failed for target: {}", selected_exec.name);
    //                                 break;
    //                             }
    //                         }
    //                         println!("Build completed.");
    //                     }
    //                     BuildAction::Quit => {}
    //                 }
    //                 break;
    //             }
    //
    //             match action {
    //                 ExecAction::QuitToBuild => break,
    //                 ExecAction::Restart => continue,
    //                 ExecAction::Kill => break,
    //             }
    //         }
    //     }
    //
    //     Ok(())
    // }

    async fn build_target(
        &self,
        builder: &Builder,
        exec_info: &ExecutableInfo,
    ) -> Result<bool, anyhow::Error> {
        let target_steps = builder.generate_build_target_steps(exec_info);
        for step in target_steps {
            let result = executor::execute_step(step.commands, |_| {}, None).await?;

            if !result.success {
                eprintln!("Build failed for target: {}", exec_info.name);
                return Ok(false);
            }
        }
        Ok(true)
    }

    async fn clean_and_build_target(
        &self,
        builder: &Builder,
        exec_info: &ExecutableInfo,
    ) -> Result<bool, anyhow::Error> {
        let clean_cmd = builder.generate_clean_target_command(exec_info);
        executor::execute_step(clean_cmd, |_| {}, None).await?;

        self.build_target(builder, exec_info).await
    }

    async fn handle_action(
        &self,
        builder: &Builder,
        exec_info: &ExecutableInfo,
        build_action: tui::BuildAction,
    ) -> Result<bool, anyhow::Error> {
        use tui::BuildAction;

        match build_action {
            BuildAction::Rebuild => self.build_target(builder, exec_info).await,
            BuildAction::Clean => self.clean_and_build_target(builder, exec_info).await,
            BuildAction::Quit => Ok(false),
        }
    }
}

impl ExecContext {
    fn new() -> Self {
        Self {
            app: None,
            info: None,
        }
    }

    fn should_resume(&self, exec_info: &ExecutableInfo) -> bool {
        self.info
            .as_ref()
            .map(|info| info.path == exec_info.path)
            .unwrap_or(false)
    }

    fn store(&mut self, app: Option<App>, info: ExecutableInfo) {
        self.app = app;
        self.info = Some(info);
    }

    fn clear(&mut self) {
        self.app = None;
        self.info = None;
    }
}

impl Default for ExecRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{target::Target, Builder, Config};

    fn create_test_context() -> BuildContext {
        let root_config = Config::default();
        let root = Target {
            path: std::path::PathBuf::from("/tmp/test"),
            kind: crate::target::TargetKind::Root,
            config: root_config.clone(),
        };
        let builder = Builder::new(root, "debug".to_string());
        BuildContext::new(builder)
    }

    #[test]
    fn test_stage_runner_creation() {
        let ctx = create_test_context();
        let runner = StageRunner::new(ctx);
        assert!(runner.get_status(Stage::Build).is_none());
    }

    #[test]
    fn test_stage_runner_clear() {
        let ctx = create_test_context();
        let runner = StageRunner::new(ctx);

        {
            let mut results = runner.results.lock().unwrap();
            results.insert(
                Stage::Build,
                StageResult::Success {
                    duration: 1.0,
                    steps_executed: 1,
                },
            );
        }

        runner.clear();

        assert!(runner.get_result(Stage::Build).is_none());
    }
}
