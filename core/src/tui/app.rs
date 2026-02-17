use super::input::{handle_key_event, InputAction};
use super::keybinding_manager::{KeyBindingManager, KeyPress};
use super::tabs::console::ConsoleTab;
use super::tabs::history::HistoryTab;
use super::tabs::performance::PerformanceTab;
use super::tabs::summary::SummaryTab;
use super::tabs::warnings::WarningsTab;
use super::tabs::TabId;
use super::vim::{CommandResult, InputMode, VimCommandMode};
use crate::builder::{Builder, ExecutableInfo};
use crate::editor::{copy_to_clipboard, Editor};
use crate::executor::{MetricHistory, StepUpdate};
use crate::history::{
    BuildHistory, BuildHistoryEntry, BuildStepResult, ExecutionHistory, ExecutionHistoryEntry,
};
use crate::monitor::ResourceMonitor;
use crate::parser::entry::{LogEntry, LogLevel};
use crate::parser::filters::LogFilter;
use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{block::Title, Block, BorderType, Borders, ListState, Paragraph},
    Frame, Terminal,
};
use std::collections::HashMap;
use std::io;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildAction {
    Quit,
    Rebuild,
    Clean,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Build,
    Exec,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecAction {
    QuitToBuild,
    Restart,
    Kill,
}

pub struct App {
    current_tab: TabId,
    log_entries: Vec<LogEntry>,
    build_steps: Vec<BuildStepResult>,
    build_complete: bool,
    build_duration: Option<f64>,
    current_step: Option<String>,
    steps_completed: usize,
    total_steps: usize,
    start_time: Instant,
    vim_mode: VimCommandMode,
    active_filter: Option<Box<dyn LogFilter>>,
    search_pattern: Option<String>,
    build_history: BuildHistory,
    editor: Editor,
    log_rx: mpsc::UnboundedReceiver<LogEntry>,
    step_rx: mpsc::UnboundedReceiver<StepUpdate>,
    build_menu_open: bool,
    exec_menu_open: bool,
    exec_menu_selection: usize,
    build_action: Option<BuildAction>,
    resource_monitor: ResourceMonitor,
    console_scroll_state: ListState,
    warnings_scroll_state: ListState,
    auto_scroll: bool,
    console_viewport_height: u16,
    cached_filtered_log_count: usize,
    filter_cache_dirty: bool,
    keybinding_manager: KeyBindingManager,
    mode: AppMode,
    exec_info: Option<ExecutableInfo>,
    exec_logs: Vec<LogEntry>,
    exec_metrics: HashMap<String, MetricHistory>,
    exec_pid: Option<u32>,
    exec_start_time: Option<Instant>,
    exec_duration: Option<f64>,
    exec_complete: bool,
    exec_action: Option<ExecAction>,
    exec_exit_code: Option<i32>,
    exec_failure_reason: Option<String>,
    selected_executable: Option<ExecutableInfo>,
    exec_history: Option<ExecutionHistory>,
    builder: Builder,
}

impl App {
    pub fn new(
        build_history: BuildHistory,
        log_rx: mpsc::UnboundedReceiver<LogEntry>,
        step_rx: mpsc::UnboundedReceiver<StepUpdate>,
        resource_monitor: ResourceMonitor,
        builder: Builder,
    ) -> Self {
        let root = builder.root();
        let config = &root.config;

        let editor = Editor::new(
            config.editor.command.clone(),
            config.editor.vscode_integration,
        );

        let leader_key = KeyPress::from_string(&config.keybindings.leader_key)
            .unwrap_or_else(|| KeyPress::from_char(' '));

        let keybinding_manager = KeyBindingManager::new(
            leader_key,
            config.keybindings.sequence_timeout_ms,
            config.keybindings.enable_leader,
        );

        Self {
            current_tab: TabId::Console,
            log_entries: Vec::new(),
            build_steps: Vec::new(),
            build_complete: false,
            build_duration: None,
            current_step: None,
            steps_completed: 0,
            total_steps: 0,
            start_time: Instant::now(),
            vim_mode: VimCommandMode::new(),
            active_filter: None,
            search_pattern: None,
            build_history,
            editor,
            log_rx,
            step_rx,
            build_menu_open: false,
            exec_menu_open: false,
            exec_menu_selection: 0,
            build_action: None,
            resource_monitor,
            console_scroll_state: ListState::default(),
            warnings_scroll_state: ListState::default(),
            auto_scroll: true,
            console_viewport_height: 20,
            cached_filtered_log_count: 0,
            filter_cache_dirty: true,
            keybinding_manager,
            mode: AppMode::Build,
            exec_info: None,
            exec_logs: Vec::new(),
            exec_metrics: HashMap::new(),
            exec_pid: None,
            exec_start_time: None,
            exec_duration: None,
            exec_complete: false,
            exec_action: None,
            exec_exit_code: None,
            exec_failure_reason: None,
            selected_executable: None,
            exec_history: None,
            builder,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        terminal.clear()?;

        let mut last_render = Instant::now();
        let render_throttle = Duration::from_millis(100);

        loop {
            if last_render.elapsed() >= render_throttle {
                terminal.draw(|f| self.render(f))?;
                last_render = Instant::now();
            }

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if self.handle_key(key).await? {
                        break;
                    }
                }
            }

            match self.mode {
                AppMode::Build => self.process_build_updates()?,
                AppMode::Exec => self.process_exec_updates()?,
            }

            if self.build_complete || self.exec_complete {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        Ok(())
    }

    fn handle_step_update(&mut self, update: StepUpdate) {
        match update {
            StepUpdate::Started(description) => {
                self.current_step = Some(description);
                self.total_steps = self.total_steps.max(self.steps_completed + 1);
            }
            StepUpdate::Progress(_msg) => {}
            StepUpdate::Finished(result) => {
                let error_count = self
                    .log_entries
                    .iter()
                    .filter(|e| e.level == LogLevel::Error)
                    .count();
                let warning_count = self
                    .log_entries
                    .iter()
                    .filter(|e| e.level == LogLevel::Warning)
                    .count();

                let step = BuildStepResult {
                    description: self.current_step.clone().unwrap_or_default(),
                    duration: result.duration,
                    success: result.success,
                    error_count,
                    warning_count,
                };

                self.build_steps.push(step);
                self.steps_completed += 1;

                if !result.success {
                    self.build_complete = true;
                }
            }
            StepUpdate::ProcessStarted(pid) => {
                self.resource_monitor.add_pid(pid);
            }
            StepUpdate::ProcessFinished(pid) => {
                self.resource_monitor.remove_pid(pid);
            }
            StepUpdate::Metric(_) => {}
        }
    }

    fn process_build_updates(&mut self) -> Result<()> {
        let mut logs_changed = false;
        while let Ok(entry) = self.log_rx.try_recv() {
            let max_log_lines = self.builder.root().config.display.max_log_lines;
            if self.log_entries.len() < max_log_lines {
                self.log_entries.push(entry);
            } else {
                self.log_entries.remove(0);
                self.log_entries.push(entry);
            }
            logs_changed = true;
        }
        if logs_changed {
            self.filter_cache_dirty = true;
        }

        let mut channel_closed = false;
        loop {
            match self.step_rx.try_recv() {
                Ok(update) => {
                    self.handle_step_update(update);
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    channel_closed = true;
                    break;
                }
            }
        }

        if channel_closed && !self.build_complete {
            self.build_complete = true;
            self.build_duration = Some(self.start_time.elapsed().as_secs_f64());
        }

        Ok(())
    }

    fn process_exec_updates(&mut self) -> Result<()> {
        let mut logs_changed = false;
        while let Ok(entry) = self.log_rx.try_recv() {
            let max_log_lines = self.builder.root().config.display.max_log_lines;
            if self.exec_logs.len() < max_log_lines {
                self.exec_logs.push(entry);
            } else {
                self.exec_logs.remove(0);
                self.exec_logs.push(entry);
            }
            logs_changed = true;
        }
        if logs_changed {
            self.filter_cache_dirty = true;
        }

        let mut channel_closed = false;
        loop {
            match self.step_rx.try_recv() {
                Ok(StepUpdate::ProcessStarted(pid)) => {
                    self.exec_pid = Some(pid);
                    self.resource_monitor.add_pid(pid);
                }
                Ok(StepUpdate::ProcessFinished(_pid)) => {
                    if let Some(pid) = self.exec_pid {
                        self.resource_monitor.remove_pid(pid);
                    }
                    self.exec_complete = true;
                    self.exec_duration = self.exec_start_time.map(|t| t.elapsed().as_secs_f64());
                }
                Ok(StepUpdate::Metric(metric)) => {
                    let key = format!("{}:{}", metric.category, metric.key);

                    if let Some(value) = metric.parse_numeric_value() {
                        self.exec_metrics
                            .entry(key.clone())
                            .or_insert_with(|| {
                                MetricHistory::new(
                                    metric.category.clone(),
                                    metric.key.clone(),
                                    metric.metric_type(),
                                    metric.visualization(),
                                )
                            })
                            .add_value(value, metric.timestamp);
                    }
                }
                Ok(StepUpdate::Finished(result)) => {
                    self.exec_exit_code = result.exit_code;
                    self.exec_failure_reason = result.failure_reason;
                }
                Ok(_) => {}
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    channel_closed = true;
                    break;
                }
            }
        }

        if channel_closed && !self.exec_complete {
            self.exec_complete = true;
            self.exec_duration = self.exec_start_time.map(|t| t.elapsed().as_secs_f64());
        }

        Ok(())
    }

    async fn handle_key(&mut self, key: event::KeyEvent) -> Result<bool> {
        match self.mode {
            AppMode::Build => self.handle_build_key(key).await,
            AppMode::Exec => self.handle_exec_key(key).await,
        }
    }

    async fn handle_build_key(&mut self, key: event::KeyEvent) -> Result<bool> {
        if self.exec_menu_open {
            let should_quit = self.handle_exec_menu_key(key).await?;
            if should_quit {
                return Ok(true);
            }
            return Ok(false);
        }

        if self.build_menu_open {
            let should_quit = self.handle_build_menu_key(key).await?;
            if should_quit {
                return Ok(true);
            }
        }

        let is_command = self.vim_mode.mode == InputMode::Command;
        let is_search = self.vim_mode.mode == InputMode::Search;

        let action = handle_key_event(
            key,
            &mut self.vim_mode,
            &self.keybinding_manager,
            is_command,
            is_search,
        );

        match action {
            InputAction::Quit => return Ok(true),
            InputAction::SwitchTab(index) => {
                if let Some(tab) = TabId::from_index(index) {
                    self.current_tab = tab;
                }
            }
            InputAction::NextTab => {
                self.current_tab = self.current_tab.next();
            }
            InputAction::PrevTab => {
                self.current_tab = self.current_tab.prev();
            }
            InputAction::EnterCommand => self.vim_mode.enter_command_mode(),
            InputAction::EnterSearch => self.vim_mode.enter_search_mode(),
            InputAction::ExecuteCommand => {
                if let Some(result) = self.vim_mode.execute_command() {
                    self.handle_command_result(result).await?;
                }
            }
            InputAction::ExecuteSearch => {
                if let Some(result) = self.vim_mode.execute_search() {
                    self.handle_command_result(result).await?;
                }
            }
            InputAction::CancelInput => self.vim_mode.exit_to_normal(),
            InputAction::InsertChar(c) => self.vim_mode.push_char(c),
            InputAction::Backspace => self.vim_mode.pop_char(),
            InputAction::NextSearch => {
                if self.current_tab == TabId::Console && self.search_pattern.is_some() {
                    let current = self.console_scroll_state.selected().unwrap_or(0);
                    if let Some(next_idx) = self.find_next_search_match(current) {
                        self.console_scroll_state.select(Some(next_idx));
                        self.auto_scroll = false;
                    }
                }
            }
            InputAction::PrevSearch => {
                if self.current_tab == TabId::Console && self.search_pattern.is_some() {
                    let current = self.console_scroll_state.selected().unwrap_or(0);
                    if let Some(prev_idx) = self.find_prev_search_match(current) {
                        self.console_scroll_state.select(Some(prev_idx));
                        self.auto_scroll = false;
                    }
                }
            }
            InputAction::OpenFile => self.open_current_file()?,
            InputAction::YankLine => self.yank_current_line()?,
            InputAction::OpenBuildMenu => {
                self.build_menu_open = !self.build_menu_open;
            }
            InputAction::OpenExecMenu => {
                self.exec_menu_open = !self.exec_menu_open;
                self.exec_menu_selection = 0;
            }
            InputAction::ScrollUp => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_up(1);
                }
            }
            InputAction::ScrollDown => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_down(1);
                }
            }
            InputAction::ScrollPageUp => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_page_up();
                }
            }
            InputAction::ScrollPageDown => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_page_down();
                }
            }
            InputAction::ScrollHalfPageUp => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_half_page_up();
                }
            }
            InputAction::ScrollHalfPageDown => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_half_page_down();
                }
            }
            InputAction::ScrollToTop => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_to_top();
                }
            }
            InputAction::ScrollToBottom => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_to_bottom();
                }
            }
            InputAction::ScrollToMiddle => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_to_middle();
                }
            }
            InputAction::ScrollToViewportTop => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_to_viewport_top();
                }
            }
            InputAction::ScrollToViewportMiddle => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_to_viewport_middle();
                }
            }
            InputAction::ScrollToViewportBottom => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_to_viewport_bottom();
                }
            }
            InputAction::ScrollUpCount(count) => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_up(count);
                }
            }
            InputAction::ScrollDownCount(count) => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_down(count);
                }
            }
            InputAction::ScrollPageUpCount(count) => {
                if self.current_tab == TabId::Console {
                    for _ in 0..count {
                        self.scroll_console_page_up();
                    }
                }
            }
            InputAction::ScrollPageDownCount(count) => {
                if self.current_tab == TabId::Console {
                    for _ in 0..count {
                        self.scroll_console_page_down();
                    }
                }
            }
            InputAction::ScrollHalfPageUpCount(count) => {
                if self.current_tab == TabId::Console {
                    for _ in 0..count {
                        self.scroll_console_half_page_up();
                    }
                }
            }
            InputAction::ScrollHalfPageDownCount(count) => {
                if self.current_tab == TabId::Console {
                    for _ in 0..count {
                        self.scroll_console_half_page_down();
                    }
                }
            }
            InputAction::WriteLogs => {
                self.write_logs(None)?;
            }
            InputAction::CleanBuild => {
                self.build_action = Some(BuildAction::Clean);
                return Ok(true);
            }
            InputAction::Rebuild => {
                self.build_action = Some(BuildAction::Rebuild);
                return Ok(true);
            }
            InputAction::ShowHelp => {
                self.show_help();
            }
            InputAction::RestartExec | InputAction::KillExec => {}
            InputAction::None => {}
        }

        Ok(false)
    }

    async fn handle_exec_key(&mut self, key: event::KeyEvent) -> Result<bool> {
        if self.build_menu_open {
            let should_quit = self.handle_build_menu_key(key).await?;
            if should_quit {
                return Ok(true);
            }
            return Ok(false);
        }

        let is_command = self.vim_mode.mode == InputMode::Command;
        let is_search = self.vim_mode.mode == InputMode::Search;

        if !is_command && !is_search {
            match key.code {
                event::KeyCode::Char('r') | event::KeyCode::Char('R') => {
                    if self.exec_complete {
                        self.exec_action = Some(ExecAction::Restart);
                        return Ok(true);
                    }
                    return Ok(false);
                }
                event::KeyCode::Char('k') | event::KeyCode::Char('K') => {
                    if !self.exec_complete {
                        if let Some(pid) = self.exec_pid {
                            #[cfg(unix)]
                            {
                                use nix::sys::signal::{kill, Signal};
                                use nix::unistd::Pid;
                                let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
                            }
                        }
                        self.exec_action = Some(ExecAction::Kill);
                        return Ok(true);
                    }
                    return Ok(false);
                }
                _ => {}
            }
        }

        let action = handle_key_event(
            key,
            &mut self.vim_mode,
            &self.keybinding_manager,
            is_command,
            is_search,
        );

        match action {
            InputAction::Quit => {
                self.exec_action = Some(ExecAction::QuitToBuild);
                return Ok(true);
            }
            InputAction::SwitchTab(index) => {
                if let Some(tab) = TabId::from_index(index) {
                    self.current_tab = tab;
                }
            }
            InputAction::NextTab => {
                self.current_tab = self.current_tab.next();
            }
            InputAction::PrevTab => {
                self.current_tab = self.current_tab.prev();
            }
            InputAction::EnterCommand => self.vim_mode.enter_command_mode(),
            InputAction::EnterSearch => self.vim_mode.enter_search_mode(),
            InputAction::ExecuteCommand => {
                if let Some(result) = self.vim_mode.execute_command() {
                    self.handle_command_result(result).await?;
                }
            }
            InputAction::ExecuteSearch => {
                if let Some(result) = self.vim_mode.execute_search() {
                    self.handle_command_result(result).await?;
                }
            }
            InputAction::CancelInput => self.vim_mode.exit_to_normal(),
            InputAction::InsertChar(c) => self.vim_mode.push_char(c),
            InputAction::Backspace => self.vim_mode.pop_char(),
            InputAction::NextSearch => {
                if self.current_tab == TabId::Console && self.search_pattern.is_some() {
                    let current = self.console_scroll_state.selected().unwrap_or(0);
                    if let Some(next_idx) = self.find_next_search_match(current) {
                        self.console_scroll_state.select(Some(next_idx));
                        self.auto_scroll = false;
                    }
                }
            }
            InputAction::PrevSearch => {
                if self.current_tab == TabId::Console && self.search_pattern.is_some() {
                    let current = self.console_scroll_state.selected().unwrap_or(0);
                    if let Some(prev_idx) = self.find_prev_search_match(current) {
                        self.console_scroll_state.select(Some(prev_idx));
                        self.auto_scroll = false;
                    }
                }
            }
            InputAction::ScrollUp => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_up(1);
                }
            }
            InputAction::ScrollDown => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_down(1);
                }
            }
            InputAction::ScrollPageUp => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_page_up();
                }
            }
            InputAction::ScrollPageDown => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_page_down();
                }
            }
            InputAction::ScrollHalfPageUp => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_half_page_up();
                }
            }
            InputAction::ScrollHalfPageDown => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_half_page_down();
                }
            }
            InputAction::ScrollToTop => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_to_top();
                }
            }
            InputAction::ScrollToBottom => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_to_bottom();
                }
            }
            InputAction::ScrollToMiddle => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_to_middle();
                }
            }
            InputAction::ScrollToViewportTop => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_to_viewport_top();
                }
            }
            InputAction::ScrollToViewportMiddle => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_to_viewport_middle();
                }
            }
            InputAction::ScrollToViewportBottom => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_to_viewport_bottom();
                }
            }
            InputAction::ScrollUpCount(count) => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_up(count);
                }
            }
            InputAction::ScrollDownCount(count) => {
                if self.current_tab == TabId::Console {
                    self.scroll_console_down(count);
                }
            }
            InputAction::ScrollPageUpCount(count) => {
                if self.current_tab == TabId::Console {
                    for _ in 0..count {
                        self.scroll_console_page_up();
                    }
                }
            }
            InputAction::ScrollPageDownCount(count) => {
                if self.current_tab == TabId::Console {
                    for _ in 0..count {
                        self.scroll_console_page_down();
                    }
                }
            }
            InputAction::ScrollHalfPageUpCount(count) => {
                if self.current_tab == TabId::Console {
                    for _ in 0..count {
                        self.scroll_console_half_page_up();
                    }
                }
            }
            InputAction::ScrollHalfPageDownCount(count) => {
                if self.current_tab == TabId::Console {
                    for _ in 0..count {
                        self.scroll_console_half_page_down();
                    }
                }
            }
            InputAction::OpenBuildMenu => {
                self.build_menu_open = !self.build_menu_open;
            }
            _ => {}
        }

        Ok(false)
    }

    async fn handle_command_result(&mut self, result: CommandResult) -> Result<()> {
        match result {
            CommandResult::Quit => return Ok(()),
            CommandResult::WriteLogs(file) => {
                self.write_logs(file)?;
            }
            CommandResult::ApplyFilter(filter) => {
                self.active_filter = Some(filter);
                self.filter_cache_dirty = true;
            }
            CommandResult::ClearFilter => {
                self.active_filter = None;
                self.filter_cache_dirty = true;
            }
            CommandResult::Search(pattern, filter) => {
                self.search_pattern = Some(pattern);
                self.active_filter = Some(filter);
                self.filter_cache_dirty = true;
            }
            CommandResult::GotoLine(line_number) => {
                self.goto_line(line_number);
            }
        }
        Ok(())
    }

    fn write_logs(&self, file: Option<String>) -> Result<()> {
        let logs = match self.mode {
            AppMode::Build => &self.log_entries,
            AppMode::Exec => &self.exec_logs,
        };

        let path = file.unwrap_or_else(|| {
            let prefix = match self.mode {
                AppMode::Build => "build_log",
                AppMode::Exec => "exec_log",
            };
            format!(
                "{}_{}.txt",
                prefix,
                chrono::Local::now().format("%Y%m%d_%H%M%S")
            )
        });

        let content = logs
            .iter()
            .map(|e| e.raw_line.clone())
            .collect::<Vec<_>>()
            .join("\n");

        std::fs::write(&path, content)
            .with_context(|| format!("Failed to write logs to {}", path))?;

        Ok(())
    }

    fn open_current_file(&self) -> Result<()> {
        let logs = match self.mode {
            AppMode::Build => &self.log_entries,
            AppMode::Exec => &self.exec_logs,
        };

        if let Some(entry) = logs.last() {
            if let Some(file_path) = &entry.file_path {
                self.editor.open_file(
                    std::path::Path::new(file_path),
                    entry.line_number,
                    entry.column,
                )?;
            }
        }
        Ok(())
    }

    fn yank_current_line(&self) -> Result<()> {
        let logs = match self.mode {
            AppMode::Build => &self.log_entries,
            AppMode::Exec => &self.exec_logs,
        };

        if let Some(entry) = logs.last() {
            copy_to_clipboard(&entry.raw_line)?;
        }
        Ok(())
    }

    fn show_help(&self) {}

    fn refresh_filter_cache(&mut self) {
        let logs = match self.mode {
            AppMode::Build => &self.log_entries,
            AppMode::Exec => &self.exec_logs,
        };

        self.cached_filtered_log_count = if let Some(filter) = &self.active_filter {
            logs.iter().filter(|e| filter.matches(e)).count()
        } else {
            logs.len()
        };
        self.filter_cache_dirty = false;
    }

    fn scroll_console_up(&mut self, amount: usize) {
        self.auto_scroll = false;
        let selected = self.console_scroll_state.selected().unwrap_or(0);
        let new_selected = selected.saturating_sub(amount);
        self.console_scroll_state.select(Some(new_selected));
    }

    fn scroll_console_down(&mut self, amount: usize) {
        if self.filter_cache_dirty {
            self.refresh_filter_cache();
        }
        let count = self.cached_filtered_log_count;

        if count == 0 {
            return;
        }

        let selected = self.console_scroll_state.selected().unwrap_or(0);
        let new_selected = (selected + amount).min(count.saturating_sub(1));

        if new_selected >= count.saturating_sub(1) {
            self.auto_scroll = true;
        } else {
            self.auto_scroll = false;
        }

        self.console_scroll_state.select(Some(new_selected));
    }

    fn scroll_console_to_top(&mut self) {
        self.auto_scroll = false;
        self.console_scroll_state.select(Some(0));
    }

    fn scroll_console_to_bottom(&mut self) {
        self.auto_scroll = true;
        if self.filter_cache_dirty {
            self.refresh_filter_cache();
        }
        let count = self.cached_filtered_log_count;
        if count > 0 {
            self.console_scroll_state
                .select(Some(count.saturating_sub(1)));
        }
    }

    fn goto_line(&mut self, line_number: usize) {
        if self.current_tab != TabId::Console {
            return;
        }

        if self.filter_cache_dirty {
            self.refresh_filter_cache();
        }
        let count = self.cached_filtered_log_count;

        if count == 0 {
            return;
        }

        if line_number == 0 {
            return;
        }

        let target_line = line_number.saturating_sub(1).min(count.saturating_sub(1));
        self.console_scroll_state.select(Some(target_line));
        self.auto_scroll = false;
    }

    fn scroll_console_page_up(&mut self) {
        let page_size = self.console_viewport_height.saturating_sub(2) as usize;
        self.scroll_console_up(page_size.max(1));
    }

    fn scroll_console_page_down(&mut self) {
        let page_size = self.console_viewport_height.saturating_sub(2) as usize;
        self.scroll_console_down(page_size.max(1));
    }

    fn scroll_console_half_page_up(&mut self) {
        let half_page = (self.console_viewport_height / 2).max(1) as usize;
        self.scroll_console_up(half_page);
    }

    fn scroll_console_half_page_down(&mut self) {
        let half_page = (self.console_viewport_height / 2).max(1) as usize;
        self.scroll_console_down(half_page);
    }

    fn scroll_console_to_middle(&mut self) {
        if self.filter_cache_dirty {
            self.refresh_filter_cache();
        }
        let count = self.cached_filtered_log_count;
        if count > 0 {
            let middle = count / 2;
            self.console_scroll_state.select(Some(middle));
            self.auto_scroll = false;
        }
    }

    fn scroll_console_to_viewport_top(&mut self) {
        let selected = self.console_scroll_state.selected().unwrap_or(0);
        let viewport_height = self.console_viewport_height.saturating_sub(2) as usize;
        let target = selected.saturating_sub(viewport_height);
        self.console_scroll_state.select(Some(target));
        self.auto_scroll = false;
    }

    fn scroll_console_to_viewport_middle(&mut self) {
        let selected = self.console_scroll_state.selected().unwrap_or(0);
        let half_viewport = (self.console_viewport_height / 2) as usize;
        let target = selected.saturating_sub(half_viewport / 2);
        self.console_scroll_state.select(Some(target));
        self.auto_scroll = false;
    }

    fn scroll_console_to_viewport_bottom(&mut self) {
        let selected = self.console_scroll_state.selected().unwrap_or(0);
        let viewport_height = self.console_viewport_height.saturating_sub(2) as usize;
        if self.filter_cache_dirty {
            self.refresh_filter_cache();
        }
        let count = self.cached_filtered_log_count;
        let target = (selected + viewport_height).min(count.saturating_sub(1));
        self.console_scroll_state.select(Some(target));
        self.auto_scroll = false;
    }

    fn find_next_search_match(&self, start_from: usize) -> Option<usize> {
        let pattern = self.search_pattern.as_ref()?;
        let logs = match self.mode {
            AppMode::Build => &self.log_entries,
            AppMode::Exec => &self.exec_logs,
        };

        if let Some(filter) = &self.active_filter {
            let mut current_idx = 0;
            for entry in logs.iter() {
                if filter.matches(entry) {
                    if current_idx > start_from
                        && (entry.message.contains(pattern.as_str())
                            || entry.raw_line.contains(pattern.as_str()))
                    {
                        return Some(current_idx);
                    }
                    current_idx += 1;
                }
            }
        } else {
            for (idx, entry) in logs.iter().enumerate().skip(start_from + 1) {
                if entry.message.contains(pattern.as_str())
                    || entry.raw_line.contains(pattern.as_str())
                {
                    return Some(idx);
                }
            }
        }

        None
    }

    fn find_prev_search_match(&self, start_from: usize) -> Option<usize> {
        let pattern = self.search_pattern.as_ref()?;
        let logs = match self.mode {
            AppMode::Build => &self.log_entries,
            AppMode::Exec => &self.exec_logs,
        };

        if let Some(filter) = &self.active_filter {
            let mut matches = Vec::new();
            let mut current_idx = 0;
            for entry in logs.iter() {
                if filter.matches(entry) {
                    if entry.message.contains(pattern.as_str())
                        || entry.raw_line.contains(pattern.as_str())
                    {
                        matches.push(current_idx);
                    }
                    current_idx += 1;
                }
            }
            matches.into_iter().rev().find(|&idx| idx < start_from)
        } else {
            for idx in (0..start_from).rev() {
                if let Some(entry) = logs.get(idx) {
                    if entry.message.contains(pattern.as_str())
                        || entry.raw_line.contains(pattern.as_str())
                    {
                        return Some(idx);
                    }
                }
            }
            None
        }
    }

    fn update_console_scroll(&mut self) {
        if self.auto_scroll {
            if self.filter_cache_dirty {
                self.refresh_filter_cache();
            }
            let count = self.cached_filtered_log_count;
            if count > 0 {
                self.console_scroll_state
                    .select(Some(count.saturating_sub(1)));
            }
        }
    }

    fn render(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(frame.size());

        self.render_header(frame, chunks[0]);

        let split_panel = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(80), Constraint::Percentage(20)])
            .split(chunks[1]);

        self.render_current_tab(frame, split_panel[0]);
        self.render_status_panel(frame, split_panel[1]);
        self.render_footer(frame, chunks[2]);
    }

    fn render_header(&self, frame: &mut Frame, area: Rect) {
        let title = match self.mode {
            AppMode::Build => Span::styled(
                " Ignis Build ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            AppMode::Exec => {
                let exec_name = self
                    .exec_info
                    .as_ref()
                    .map(|e| e.name.as_str())
                    .unwrap_or("Unknown");
                Span::styled(
                    format!(" Ignis Exec: {} ", exec_name),
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                )
            }
        };

        let tabs = [
            ("[Alt+1] Console", TabId::Console),
            ("[Alt+2] Summary", TabId::Summary),
            ("[Alt+3] Performance", TabId::Performance),
            ("[Alt+4] Warnings", TabId::Warnings),
            ("[Alt+5] History", TabId::History),
        ];

        let tab_spans: Vec<Span> = tabs
            .iter()
            .flat_map(|(label, id)| {
                let style = if *id == self.current_tab {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                vec![Span::raw(" "), Span::styled(*label, style)]
            })
            .collect();

        let header = Paragraph::new(Line::from(
            std::iter::once(title)
                .chain(IntoIterator::into_iter(tab_spans))
                .collect::<Vec<_>>(),
        ))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan)),
        );

        frame.render_widget(header, area);
    }

    fn render_current_tab(&mut self, frame: &mut Frame, area: Rect) {
        match self.mode {
            AppMode::Build => self.render_build_tab(frame, area),
            AppMode::Exec => self.render_exec_tab(frame, area),
        }
    }

    fn render_build_tab(&mut self, frame: &mut Frame, area: Rect) {
        match self.current_tab {
            TabId::Console => {
                self.console_viewport_height = area.height.saturating_sub(2);
                self.update_console_scroll();
                let tab = ConsoleTab::new(
                    &self.log_entries,
                    self.active_filter.as_ref(),
                    self.search_pattern.as_deref(),
                );
                tab.render(frame, area, &mut self.console_scroll_state);
            }
            TabId::Summary => {
                let tab = SummaryTab::new(&self.build_steps);
                tab.render(frame, area);
            }
            TabId::Performance => {
                let elapsed = self
                    .build_duration
                    .unwrap_or_else(|| self.start_time.elapsed().as_secs_f64());
                let resource_stats = self.resource_monitor.get_stats();
                let tab = PerformanceTab::new(
                    &self.build_steps,
                    elapsed,
                    self.build_complete,
                    resource_stats,
                );
                tab.render(frame, area);
            }
            TabId::Warnings => {
                let tab = WarningsTab::new(&self.log_entries);
                tab.render(frame, area, &mut self.warnings_scroll_state);
            }
            TabId::History => {
                let tab = HistoryTab::new(self.build_history.entries());
                tab.render(frame, area);
            }
        }
    }

    fn render_exec_tab(&mut self, frame: &mut Frame, area: Rect) {
        match self.current_tab {
            TabId::Console => {
                self.console_viewport_height = area.height.saturating_sub(2);
                self.update_console_scroll();
                let tab = ConsoleTab::new(
                    &self.exec_logs,
                    self.active_filter.as_ref(),
                    self.search_pattern.as_deref(),
                );
                tab.render(frame, area, &mut self.console_scroll_state);
            }
            TabId::Summary => {
                let tab = SummaryTab::new(&[]);
                tab.render_metrics(frame, area, &self.exec_metrics);
            }
            TabId::Performance => {
                let elapsed = self.exec_duration.unwrap_or_else(|| {
                    self.exec_start_time
                        .map(|t| t.elapsed().as_secs_f64())
                        .unwrap_or(0.0)
                });
                let resource_stats = self.resource_monitor.get_stats();
                let tab = PerformanceTab::new_runtime(elapsed, self.exec_complete, resource_stats);
                tab.render(frame, area);
            }
            TabId::Warnings => {
                let tab = WarningsTab::new(&self.exec_logs);
                tab.render(frame, area, &mut self.warnings_scroll_state);
            }
            TabId::History => {
                if let Some(exec_history) = &self.exec_history {
                    HistoryTab::render_exec_history(exec_history.entries(), frame, area);
                } else {
                    let message = Paragraph::new("History not available")
                        .block(Block::default().borders(Borders::ALL));
                    frame.render_widget(message, area);
                }
            }
        }
    }

    fn render_status_panel(&self, frame: &mut Frame, area: Rect) {
        match self.mode {
            AppMode::Build => self.render_build_status(frame, area),
            AppMode::Exec => self.render_exec_status(frame, area),
        }
    }

    fn render_build_status(&self, frame: &mut Frame, area: Rect) {
        if self.exec_menu_open {
            let executables = self.builder.find_executables();

            let mut lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Executables",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
            ];

            if executables.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "No executables found",
                    Style::default().fg(Color::DarkGray),
                )));
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    "(Only projects with is_engine=false have executables)",
                    Style::default().fg(Color::DarkGray),
                )));
                lines.push(Line::from(""));
            } else {
                for (idx, exec_info) in executables.iter().enumerate() {
                    let is_selected = idx == self.exec_menu_selection;

                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![
                        Span::raw(if is_selected { " > " } else { "   " }),
                        Span::styled(
                            &exec_info.name,
                            if is_selected {
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(Color::White)
                            },
                        ),
                    ]));

                    if is_selected {
                        let project_name = exec_info
                            .project_dir
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown");
                        lines.push(Line::from(vec![
                            Span::raw("     "),
                            Span::styled(
                                format!("({})", project_name),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));
                    }
                }

                lines.push(Line::from(""));
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::styled(" [j/k ↓↑] ", Style::default().fg(Color::Cyan)),
                    Span::raw("Navigate"),
                ]));
                lines.push(Line::from(vec![
                    Span::styled(
                        " [o/Enter] ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("Open"),
                ]));
                lines.push(Line::from(vec![
                    Span::styled(
                        " [c] ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("Clean build"),
                ]));
            }

            lines.push(Line::from(vec![
                Span::styled(" [Esc] ", Style::default().fg(Color::Cyan)),
                Span::raw("Close menu"),
            ]));

            let paragraph = Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(Title::from(" Exec Menu ").alignment(Alignment::Center)),
            );

            frame.render_widget(paragraph, area);
            return;
        }

        if self.build_menu_open {
            let lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Build Options",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        " [R] ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("Rebuild"),
                ]),
                Line::from(vec![
                    Span::raw("     "),
                    Span::styled("Start a new build", Style::default().fg(Color::DarkGray)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        " [C] ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("Clean"),
                ]),
                Line::from(vec![
                    Span::raw("     "),
                    Span::styled(
                        "Clean build artifacts",
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled(" [Esc] ", Style::default().fg(Color::Cyan)),
                    Span::raw("Close menu"),
                ]),
            ];

            let paragraph = Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(Title::from(" Build Menu ").alignment(Alignment::Center)),
            );

            frame.render_widget(paragraph, area);
            return;
        }

        let elapsed = self
            .build_duration
            .unwrap_or_else(|| self.start_time.elapsed().as_secs_f64());
        let error_count = self
            .log_entries
            .iter()
            .filter(|e| e.level == LogLevel::Error)
            .count();
        let warning_count = self
            .log_entries
            .iter()
            .filter(|e| e.level == LogLevel::Warning)
            .count();

        let percentage = if self.total_steps > 0 {
            (self.steps_completed as f64 / self.total_steps as f64) * 100.0
        } else {
            0.0
        };

        let lines = vec![
            Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    if self.build_complete {
                        "Complete"
                    } else {
                        "Building"
                    },
                    Style::default().fg(if self.build_complete {
                        Color::Green
                    } else {
                        Color::Cyan
                    }),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Progress: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!(
                        "{}/{} ({:.0}%)",
                        self.steps_completed, self.total_steps, percentage
                    ),
                    Style::default().fg(Color::Green),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Current: ", Style::default().fg(Color::Yellow)),
                Span::raw(self.current_step.as_deref().unwrap_or("Idle")),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Elapsed: ", Style::default().fg(Color::Yellow)),
                Span::styled(format!("{:.1}s", elapsed), Style::default().fg(Color::Cyan)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Errors: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    error_count.to_string(),
                    Style::default().fg(if error_count > 0 {
                        Color::Red
                    } else {
                        Color::Green
                    }),
                ),
            ]),
            Line::from(vec![
                Span::styled("Warnings: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    warning_count.to_string(),
                    Style::default().fg(if warning_count > 0 {
                        Color::Yellow
                    } else {
                        Color::Green
                    }),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Legend:",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(vec![
                Span::styled("  E", Style::default().fg(Color::Red)),
                Span::styled(" = Errors", Style::default().fg(Color::DarkGray)),
            ]),
            Line::from(vec![
                Span::styled("  W", Style::default().fg(Color::Yellow)),
                Span::styled(" = Warnings", Style::default().fg(Color::DarkGray)),
            ]),
        ];

        let paragraph = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan))
                .title(Title::from(" Status ").alignment(Alignment::Center)),
        );

        frame.render_widget(paragraph, area);
    }

    fn render_exec_status(&self, frame: &mut Frame, area: Rect) {
        if self.build_menu_open {
            let lines = vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Build Options",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )),
                Line::from(""),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        " [R] ",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("Rebuild"),
                ]),
                Line::from(vec![
                    Span::raw("     "),
                    Span::styled("Start a new build", Style::default().fg(Color::DarkGray)),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled(
                        " [C] ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("Clean"),
                ]),
                Line::from(vec![
                    Span::raw("     "),
                    Span::styled(
                        "Clean build artifacts",
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
                Line::from(""),
                Line::from(vec![
                    Span::styled(" [Esc] ", Style::default().fg(Color::Cyan)),
                    Span::raw("Close menu"),
                ]),
            ];

            let paragraph = Paragraph::new(lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::Green))
                    .title(Title::from(" Build Options ").alignment(Alignment::Center)),
            );

            frame.render_widget(paragraph, area);
            return;
        }

        let elapsed = self.exec_duration.unwrap_or_else(|| {
            self.exec_start_time
                .map(|t| t.elapsed().as_secs_f64())
                .unwrap_or(0.0)
        });

        let exec_name = self
            .exec_info
            .as_ref()
            .map(|e| e.name.as_str())
            .unwrap_or("Unknown");

        let error_count = self
            .exec_logs
            .iter()
            .filter(|e| e.level == LogLevel::Error)
            .count();
        let warning_count = self
            .exec_logs
            .iter()
            .filter(|e| e.level == LogLevel::Warning)
            .count();

        let mut lines = vec![
            Line::from(vec![
                Span::styled("Program: ", Style::default().fg(Color::Yellow)),
                Span::styled(exec_name, Style::default().fg(Color::Cyan)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    if self.exec_complete {
                        "Finished"
                    } else {
                        "Running"
                    },
                    Style::default().fg(if self.exec_complete {
                        Color::Red
                    } else {
                        Color::Green
                    }),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Runtime: ", Style::default().fg(Color::Yellow)),
                Span::styled(format!("{:.1}s", elapsed), Style::default().fg(Color::Cyan)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("PID: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    self.exec_pid
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "N/A".to_string()),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
        ];

        if let Some(exit_code) = self.exec_exit_code {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("Exit Code: ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    exit_code.to_string(),
                    Style::default().fg(if exit_code == 0 {
                        Color::Green
                    } else {
                        Color::Red
                    }),
                ),
            ]));
        }

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Errors: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                error_count.to_string(),
                Style::default().fg(if error_count > 0 {
                    Color::Red
                } else {
                    Color::Green
                }),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Warnings: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                warning_count.to_string(),
                Style::default().fg(if warning_count > 0 {
                    Color::Yellow
                } else {
                    Color::Green
                }),
            ),
        ]));

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("Metrics: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                self.exec_metrics.len().to_string(),
                Style::default().fg(Color::Cyan),
            ),
        ]));

        let paragraph = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Magenta))
                .title(Title::from(" Runtime Status ").alignment(Alignment::Center)),
        );

        frame.render_widget(paragraph, area);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let mode_text = match self.mode {
            AppMode::Build => {
                if self.exec_menu_open {
                    "Exec Menu: [j/k/↓/↑]: Navigate | [o/Enter]: Open | [c]: Clean | [Q | Esc]: Close Menu"
                        .to_string()
                } else if self.build_menu_open {
                    "Build Options: [R]: Rebuild | [C]: Clean | [Q | Esc]: Close Menu".to_string()
                } else {
                    match self.vim_mode.mode {
                        InputMode::Normal => {
                            if let Some(count) = self.vim_mode.get_count_display() {
                                format!("{}", count)
                            } else if let Some(sequence) = self.vim_mode.get_sequence_display() {
                                format!("Keys: {} (waiting for next key...)", sequence)
                            } else {
                                "q: Quit | <Space>: Leader | b: Build | e: Exec | Alt+[1-5]: Tabs | H/L: Tab Nav | :: Cmd | /: Search | [num]j/k/Ctrl+U/D: Nav".to_string()
                            }
                        }
                        InputMode::Command => format!(":{}", self.vim_mode.input_buffer),
                        InputMode::Search => format!("/{}", self.vim_mode.input_buffer),
                    }
                }
            }
            AppMode::Exec => {
                if self.build_menu_open {
                    "Build Options: [R]: Rebuild | [C]: Clean | [Q | Esc]: Close Menu".to_string()
                } else {
                    match self.vim_mode.mode {
                        InputMode::Normal => {
                            if self.exec_complete {
                                "q: Back to Build | r: Restart | b: Build Options | Alt+[1-5]: Tabs"
                                    .to_string()
                            } else {
                                "q: Back to Build | k: Kill | b: Build Options | Alt+[1-5]: Tabs"
                                    .to_string()
                            }
                        }
                        InputMode::Command => format!(":{}", self.vim_mode.input_buffer),
                        InputMode::Search => format!("/{}", self.vim_mode.input_buffer),
                    }
                }
            }
        };

        let style = if self.vim_mode.has_count() {
            Style::default().fg(Color::Green)
        } else if self.vim_mode.pending_sequence.is_some() {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Cyan)
        };

        let footer = Paragraph::new(mode_text).style(style).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan)),
        );

        frame.render_widget(footer, area);
    }

    async fn handle_build_menu_key(&mut self, key: event::KeyEvent) -> Result<bool> {
        use crossterm::event::KeyCode;

        match key.code {
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.build_action = Some(BuildAction::Rebuild);
                return Ok(true);
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                self.build_action = Some(BuildAction::Clean);
                return Ok(true);
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.build_menu_open = false;
            }
            _ => {}
        }

        Ok(false)
    }

    async fn handle_exec_menu_key(&mut self, key: event::KeyEvent) -> Result<bool> {
        use crossterm::event::KeyCode;

        let executables = self.builder.find_executables();

        if executables.is_empty() {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    self.exec_menu_open = false;
                }
                _ => {}
            }
            return Ok(false);
        }

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if self.exec_menu_selection < executables.len().saturating_sub(1) {
                    self.exec_menu_selection += 1;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.exec_menu_selection > 0 {
                    self.exec_menu_selection -= 1;
                }
            }
            KeyCode::Char('o') | KeyCode::Char('O') | KeyCode::Enter => {
                if let Some(exec_info) = executables.get(self.exec_menu_selection) {
                    self.selected_executable = Some(exec_info.clone());
                    self.exec_menu_open = false;
                    return Ok(true);
                }
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                if let Some(exec_info) = executables.get(self.exec_menu_selection) {
                    let _ = std::fs::remove_dir_all(&exec_info.build_dir);
                    let _ = std::fs::remove_dir_all(&exec_info.install_dir);
                    self.exec_menu_open = false;
                }
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.exec_menu_open = false;
            }
            _ => {}
        }

        Ok(false)
    }

    pub fn get_build_action(&self) -> Option<BuildAction> {
        self.build_action
    }

    pub fn finalize_build(&mut self) -> Result<()> {
        let total_duration = self.start_time.elapsed().as_secs_f64();
        let mut entry = BuildHistoryEntry::new(self.builder.preset().to_string());

        for step in &self.build_steps {
            entry.add_step(step.clone());
        }

        entry.finalize(total_duration);
        self.build_history.add_entry(entry)?;

        Ok(())
    }

    pub fn finalize_exec(&mut self) -> Result<()> {
        if let (Some(exec_info), Some(exec_history)) =
            (self.exec_info.as_ref(), self.exec_history.as_mut())
        {
            let duration = self.exec_duration.unwrap_or_else(|| {
                self.exec_start_time
                    .map(|t| t.elapsed().as_secs_f64())
                    .unwrap_or(0.0)
            });

            let error_count = self
                .exec_logs
                .iter()
                .filter(|e| e.level == LogLevel::Error)
                .count();
            let warning_count = self
                .exec_logs
                .iter()
                .filter(|e| e.level == LogLevel::Warning)
                .count();
            let success = self.exec_exit_code.map(|c| c == 0).unwrap_or(false);

            let mut entry = ExecutionHistoryEntry::new(
                exec_info.name.clone(),
                exec_info.path.to_string_lossy().to_string(),
            );
            entry.duration = duration;
            entry.exit_code = self.exec_exit_code;
            entry.success = success;
            entry.error_count = error_count;
            entry.warning_count = warning_count;
            entry.metric_count = self.exec_metrics.len();
            entry.log_count = self.exec_logs.len();
            entry.failure_reason = self.exec_failure_reason.clone();

            exec_history.add_entry(entry)?;
        }

        Ok(())
    }

    pub fn new_exec_mode(
        log_rx: mpsc::UnboundedReceiver<LogEntry>,
        step_rx: mpsc::UnboundedReceiver<StepUpdate>,
        resource_monitor: ResourceMonitor,
        exec_info: ExecutableInfo,
        builder: Builder,
    ) -> Self {
        let root = builder.root();
        let editor = Editor::new(
            root.config.editor.command.clone(),
            root.config.editor.vscode_integration,
        );

        let leader_key = KeyPress::from_string(&root.config.keybindings.leader_key)
            .unwrap_or_else(|| KeyPress::from_char(' '));

        let keybinding_manager = KeyBindingManager::new(
            leader_key,
            root.config.keybindings.sequence_timeout_ms,
            root.config.keybindings.enable_leader,
        );

        let storage_path = root.config.storage_path();

        let build_history = BuildHistory::new(storage_path.clone(), root.config.history.max_builds)
            .unwrap_or_else(|_| BuildHistory::new(std::path::PathBuf::new(), 10).unwrap());

        let exec_history = ExecutionHistory::new(storage_path, root.config.history.max_builds).ok();

        Self {
            current_tab: TabId::Console,
            log_entries: Vec::new(),
            build_steps: Vec::new(),
            build_complete: false,
            build_duration: None,
            current_step: None,
            steps_completed: 0,
            total_steps: 0,
            start_time: Instant::now(),
            vim_mode: VimCommandMode::new(),
            active_filter: None,
            search_pattern: None,
            build_history,
            editor,
            log_rx,
            step_rx,
            build_menu_open: false,
            exec_menu_open: false,
            exec_menu_selection: 0,
            build_action: None,
            resource_monitor,
            console_scroll_state: ListState::default(),
            warnings_scroll_state: ListState::default(),
            auto_scroll: true,
            console_viewport_height: 20,
            cached_filtered_log_count: 0,
            filter_cache_dirty: true,
            keybinding_manager,
            mode: AppMode::Exec,
            exec_info: Some(exec_info),
            exec_logs: Vec::new(),
            exec_metrics: HashMap::new(),
            exec_pid: None,
            exec_start_time: Some(Instant::now()),
            exec_duration: None,
            exec_complete: false,
            exec_action: None,
            exec_exit_code: None,
            exec_failure_reason: None,
            selected_executable: None,
            exec_history,
            builder,
        }
    }

    pub fn get_exec_action(&self) -> Option<ExecAction> {
        self.exec_action
    }

    pub fn get_selected_executable(&self) -> Option<ExecutableInfo> {
        self.selected_executable.clone()
    }
}
