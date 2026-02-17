use std::path::{Path, PathBuf};

use crate::{
    command::CMakeCommands,
    target::{Target, TargetKind},
};

#[derive(Debug, Clone)]
pub struct ExecutableInfo {
    pub path: PathBuf,
    pub name: String,
    pub project_dir: PathBuf,
    pub build_dir: PathBuf,
    pub install_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Builder {
    preset: String,
    root: Target,
    targets: Vec<Target>,
    cmake: CMakeCommands,
}

#[derive(Debug, Clone)]
pub struct BuildStep {
    pub description: String,
    pub commands: Vec<String>,
}

impl BuildStep {
    pub fn new(description: String, commands: Vec<String>) -> Self {
        Self {
            description,
            commands,
        }
    }
}

impl std::fmt::Display for BuildStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{}", self.description)?;
        for cmd in &self.commands {
            writeln!(f, "  {}", cmd)?;
        }
        Ok(())
    }
}

impl Builder {
    pub fn new(root: Target, preset: String) -> Self {
        let cmake = CMakeCommands::new(preset.clone(), true);
        Self {
            root,
            preset,
            targets: Vec::new(),
            cmake,
        }
    }

    pub fn with_ninja(mut self, ninja: bool) -> Self {
        self.cmake = CMakeCommands::new(self.preset.clone(), ninja);
        self
    }

    pub fn with_targets(mut self, targets: Vec<Target>) -> Self {
        self.targets = targets;
        self
    }

    pub fn preset(&self) -> &str {
        &self.preset
    }

    pub fn root(&self) -> &Target {
        &self.root
    }

    pub fn targets(&self) -> &[Target] {
        &self.targets
    }

    pub fn cmake(&self) -> &CMakeCommands {
        &self.cmake
    }

    pub fn generate_build_all(&self) -> Vec<BuildStep> {
        let mut steps: Vec<BuildStep> = Vec::new();

        for target in &self.targets {
            let (build_dir, install_dir) = self.get_dirs(target.path.clone(), &self.preset);

            let build_dir_str = build_dir.display().to_string();
            let install_dir_str = install_dir.display().to_string();

            let source_dir = target.path.display().to_string();

            steps.push(
                self.cmake
                    .configure_step("CMake", source_dir, build_dir_str.clone()),
            );

            let target_name = target.config.build.name.as_deref().unwrap_or("project");

            steps.push(self.cmake.build_step(target_name, build_dir_str.clone()));

            if target.kind == TargetKind::Installer {
                steps.push(
                    self.cmake
                        .install_step("artifacts", build_dir_str, install_dir_str),
                );
            }
        }

        steps
    }

    pub fn find_executables(&self) -> Vec<ExecutableInfo> {
        self.targets
            .iter()
            .flat_map(|target| self.find_executables_in_target(&target.path))
            .collect()
    }

    fn find_executables_in_target(&self, target_path: &Path) -> Vec<ExecutableInfo> {
        let build_dir = target_path.join("builds").join(&self.preset);

        if !build_dir.exists() {
            return Vec::new();
        }

        std::fs::read_dir(&build_dir)
            .ok()
            .into_iter()
            .flat_map(|entries| entries.flatten())
            .filter(|entry| {
                entry.file_type().ok().map(|t| t.is_file()).unwrap_or(false)
                    && is_executable(&entry.path())
            })
            .map(|entry| {
                let install_dir = target_path.join("install");
                ExecutableInfo {
                    path: entry.path(),
                    name: entry.file_name().to_string_lossy().to_string(),
                    project_dir: target_path.to_path_buf(),
                    build_dir: build_dir.clone(),
                    install_dir,
                }
            })
            .collect()
    }

    pub fn get_root_dirs(&self) -> (PathBuf, PathBuf) {
        let build_dir = self.root.path.join("builds").join(&self.preset);
        let install_dir = self.root.path.join("install");

        (build_dir, install_dir)
    }

    pub fn get_dirs(&self, target_path: PathBuf, preset: &str) -> (PathBuf, PathBuf) {
        let build_dir = target_path.join("builds").join(preset);
        let install_dir = self.root.path.join("install");

        (build_dir, install_dir)
    }

    pub fn generate_build_target_steps(&self, exec_info: &ExecutableInfo) -> Vec<BuildStep> {
        let source_dir = exec_info.project_dir.display().to_string();
        let build_dir = exec_info.build_dir.display().to_string();
        let install_dir = exec_info.install_dir.display().to_string();

        vec![
            self.cmake
                .configure_step(&exec_info.name, source_dir, build_dir.clone()),
            self.cmake
                .build_target_step(&exec_info.name, build_dir.clone(), &exec_info.name),
            self.cmake
                .install_step(&exec_info.name, build_dir, install_dir),
        ]
    }

    pub fn generate_clean_target_command(&self, exec_info: &ExecutableInfo) -> Vec<String> {
        self.cmake
            .build_target(exec_info.build_dir.display().to_string(), "clean")
    }

    pub fn generate_clean_all(&self) -> Vec<BuildStep> {
        let mut steps: Vec<BuildStep> = Vec::new();

        for target in &self.targets {
            let (build_dir, install_dir) = self.get_dirs(target.path.clone(), &self.preset);

            let build_dir_str = build_dir.display().to_string();
            let install_dir_str = install_dir.display().to_string();

            let target_name = target.config.build.name.as_deref().unwrap_or("project");

            steps.push(
                self.cmake
                    .clean_step(target_name, &[build_dir_str, install_dir_str]),
            );
        }

        steps
    }

    pub fn generate_clean_command(&self, build_dir: String, install_dir: String) -> Vec<String> {
        self.cmake.clean(&[build_dir, install_dir])
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .ok()
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    true
}

pub fn detect_available_presets(_source_dir: &Path) -> Vec<String> {
    vec!["debug".to_string(), "release".to_string()]
}
