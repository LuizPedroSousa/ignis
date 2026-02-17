use anyhow::{Context, Result};
use clap::Parser;
use ignis_core::logger::Logger;
use ignis_core::parser::{CompilerOutputParser, LogLevel};
use ignis_core::runner::BuildContext;
use ignis_core::{Builder, Cli, Config, ExecRunner, Stage, StageContext, StageRunner};

async fn execute_stages(
    builder: &Builder,
    stage_runner: &StageRunner,
    stages: Vec<Stage>,
) -> Result<ignis_core::tui::App> {
    let (mut app, log_tx, step_tx) = stage_runner.idle().await?;
    let context = StageContext::new(builder.clone(), log_tx.clone(), step_tx.clone());

    let build_handle = {
        let stage_runner_clone = StageRunner::new(BuildContext::new(builder.clone()));
        tokio::spawn(async move {
            stage_runner_clone
                .execute_with_dependencies(stages, context)
                .await
        })
    };

    let app_handle = tokio::spawn(async move {
        app.run().await?;
        Ok::<_, anyhow::Error>(app)
    });

    let _build_results = build_handle.await??;
    drop(log_tx);
    drop(step_tx);

    let mut app = app_handle.await??;
    app.finalize_build()?;

    Ok(app)
}

async fn run_with_tui(builder: Builder) -> Result<()> {
    use ignis_core::tui::BuildAction;

    let mut exec_runner = ExecRunner::new();
    let stage_runner = StageRunner::new(BuildContext::new(builder.clone()));

    let (mut app, _log_tx, _step_tx) = stage_runner.idle().await?;

    let app_handle = tokio::spawn(async move {
        app.run().await?;
        Ok::<_, anyhow::Error>(app)
    });

    let mut app = app_handle.await??;

    loop {
        if let Some(selected_exec) = app.get_selected_executable() {
            exec_runner.run(&builder, selected_exec).await?;

            let (mut new_app, _new_log_tx, _new_step_tx) = stage_runner.idle().await?;

            let app_handle = tokio::spawn(async move {
                new_app.run().await?;
                Ok::<_, anyhow::Error>(new_app)
            });

            app = app_handle.await??;
            continue;
        }

        match app.get_build_action() {
            Some(BuildAction::Quit) | None => break,
            Some(BuildAction::Rebuild) => {
                let stages = vec![Stage::Configure, Stage::Build, Stage::Install];
                app = execute_stages(&builder, &stage_runner, stages).await?;
            }
            Some(BuildAction::Clean) => {
                execute_stages(&builder, &stage_runner, vec![Stage::Clean]).await?;
                app = execute_stages(&builder, &stage_runner, vec![Stage::Configure, Stage::Build, Stage::Install]).await?;
            }
        }
    }

    Ok(())
}

async fn run_without_tui(builder: Builder) -> Result<()> {
    let logger = std::sync::Arc::new(Logger::new());
    let parser = std::sync::Arc::new(std::sync::Mutex::new(CompilerOutputParser::new()));

    logger.log(LogLevel::Info, "Starting build...");

    let steps = builder.generate_build_all();

    for step in steps {
        logger.log(LogLevel::Info, &format!("Step: {}", step));

        let logger_clone = logger.clone();
        let parser_clone = parser.clone();

        let result = ignis_core::executor::execute_step(
            step.commands,
            move |line| {
                let entry = parser_clone.lock().unwrap().parse_line(&line);
                logger_clone.log_entry(&entry);
            },
            None,
        )
        .await?;

        if !result.success {
            logger.log(ignis_core::parser::entry::LogLevel::Error, "Build failed!");
            std::process::exit(1);
        }
    }

    logger.log(
        ignis_core::parser::entry::LogLevel::Info,
        "Build completed successfully!",
    );

    Ok(())
}

fn list_presets(cli: &Cli) {
    let source_dir = cli.source_directory();
    let presets = ignis_core::builder::detect_available_presets(&source_dir);

    println!("Available presets:");
    for preset in presets {
        println!("  - {}", preset);
    }
}

fn show_history(root: &ignis_core::target::Target, count: Option<usize>) -> Result<()> {
    use ignis_core::history::BuildHistory;

    let logger = Logger::new();
    let storage_path = root.config.storage_path();
    let history = BuildHistory::new(storage_path, root.config.history.max_builds)
        .context("Failed to load history")?;

    let entries = history.entries();
    let count = count.unwrap_or(10).min(entries.len());

    if entries.is_empty() {
        logger.log(LogLevel::Info, "No build history found.");
        return Ok(());
    }

    logger.log(LogLevel::Info, &format!("Build History (last {} entries):", count));
    logger.log(LogLevel::Info, "");

    for entry in entries.iter().rev().take(count) {
        let status = if entry.success { "✓" } else { "✗" };
        logger.log(
            LogLevel::Info,
            &format!(
                "{} {} | {} | {:.1}s | {} errors, {} warnings",
                status,
                entry.timestamp.format("%Y-%m-%d %H:%M:%S"),
                entry.preset,
                entry.duration,
                entry.error_count,
                entry.warning_count
            ),
        );
    }

    Ok(())
}

fn clear_history(
    root: &ignis_core::target::Target,
    history_type: ignis_core::cli::HistoryType,
) -> Result<()> {
    use ignis_core::cli::HistoryType;
    use ignis_core::history::{BuildHistory, ExecutionHistory};

    let storage_path = root.config.storage_path();

    match history_type {
        HistoryType::Build => {
            let mut history = BuildHistory::new(storage_path, root.config.history.max_builds)
                .context("Failed to load build history")?;
            history.clear()?;
            println!("Build history cleared.");
        }
        HistoryType::Executable => {
            let mut exec_history = ExecutionHistory::new(
                storage_path.join("exec_history.json"),
                root.config.history.max_builds,
            )
            .context("Failed to load execution history")?;
            exec_history.clear()?;
            println!("Execution history cleared.");
        }
        HistoryType::All => {
            let mut history =
                BuildHistory::new(storage_path.clone(), root.config.history.max_builds)
                    .context("Failed to load build history")?;
            history.clear()?;

            let mut exec_history = ExecutionHistory::new(
                storage_path.join("exec_history.json"),
                root.config.history.max_builds,
            )
            .context("Failed to load execution history")?;
            exec_history.clear()?;

            println!("All history cleared.");
        }
    }

    Ok(())
}

async fn clean_build(builder: Builder) -> Result<()> {
    println!("Cleaning build for preset: {}", builder.preset());

    let (build_dir, install_dir) = builder.get_root_dirs();

    let build_dir = build_dir.display().to_string();
    let install_dir = install_dir.display().to_string();

    let command = builder.generate_clean_command(build_dir, install_dir);

    let result = ignis_core::executor::execute_step(
        command,
        |line| {
            println!("{}", line);
        },
        None,
    )
    .await?;

    if result.success {
        println!("Clean completed successfully.");
    } else {
        println!("Clean failed!");
        std::process::exit(1);
    }

    Ok(())
}

fn init_config(cli: &Cli, name: Option<String>) -> Result<()> {
    let target_dir = cli.source_directory();
    let config_path = target_dir.join("ignis.toml");

    if config_path.exists() {
        anyhow::bail!(
            "ignis.toml already exists at {}. Remove it first if you want to reinitialize.",
            config_path.display()
        );
    }

    let mut config = Config::default();
    config.build.name = name;

    config
        .save_to_file(&config_path)
        .context("Failed to save ignis.toml")?;

    println!("Created ignis.toml at {}", config_path.display());

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Some(command) = &cli.command {
        match command {
            ignis_core::cli::Commands::Init { name } => {
                init_config(&cli, name.clone())?;
                return Ok(());
            }
            ignis_core::cli::Commands::Presets => {
                list_presets(&cli);
                return Ok(());
            }
            _ => {}
        }
    }

    let (root, targets) = Config::load_from_cli(&cli)?;

    if let Some(command) = &cli.command {
        match command {
            ignis_core::cli::Commands::History { command } => {
                match command {
                    ignis_core::cli::HistoryCommands::Show { count } => {
                        show_history(&root, *count)?;
                    }
                    ignis_core::cli::HistoryCommands::Clear { r#type } => {
                        clear_history(&root, *r#type)?;
                    }
                }
                return Ok(());
            }
            _ => {}
        }
    }

    let preset = cli.preset.as_deref().unwrap_or("debug");
    let builder = Builder::new(root, preset.to_string()).with_targets(targets);

    if let Some(command) = &cli.command {
        match command {
            ignis_core::cli::Commands::Clean { .. } => {
                clean_build(builder).await?;
                return Ok(());
            }
            _ => {}
        }
    }

    if cli.no_tui {
        run_without_tui(builder).await?;
    } else {
        run_with_tui(builder).await?;
    }

    Ok(())
}
