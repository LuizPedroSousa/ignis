use crate::builder::BuildStep;

#[derive(Debug, Clone)]
pub struct CMakeCommands {
    preset: String,
    ninja: bool,
}

impl CMakeCommands {
    pub fn new(preset: String, ninja: bool) -> Self {
        Self { preset, ninja }
    }

    pub fn configure_step(
        &self,
        target_name: &str,
        source_dir: String,
        build_dir: String,
    ) -> BuildStep {
        BuildStep::new(
            format!("Configuring {}", target_name),
            self.configure(source_dir, build_dir),
        )
    }

    pub fn configure_step_with_prefix(
        &self,
        target_name: &str,
        source_dir: String,
        build_dir: String,
        prefix_path: Option<String>,
    ) -> BuildStep {
        BuildStep::new(
            format!("Configuring {}", target_name),
            self.configure_with_prefix(source_dir, build_dir, prefix_path),
        )
    }

    pub fn build_step(&self, target_name: &str, build_dir: String) -> BuildStep {
        BuildStep::new(format!("Building {}", target_name), self.build(build_dir))
    }

    pub fn build_target_step(
        &self,
        target_name: &str,
        build_dir: String,
        target: &str,
    ) -> BuildStep {
        BuildStep::new(
            format!("Building {}", target_name),
            self.build_target(build_dir, target),
        )
    }

    pub fn install_step(
        &self,
        target_name: &str,
        build_dir: String,
        install_dir: String,
    ) -> BuildStep {
        BuildStep::new(
            format!("Installing {}", target_name),
            self.install(build_dir, install_dir),
        )
    }

    pub fn clean_step(&self, target_name: &str, paths: &[String]) -> BuildStep {
        BuildStep::new(
            format!("Cleaning {}", target_name),
            self.clean(paths),
        )
    }

    fn configure(&self, source_dir: String, build_dir: String) -> Vec<String> {
        self.configure_with_prefix(source_dir, build_dir, None)
    }

    fn configure_with_prefix(
        &self,
        source_dir: String,
        build_dir: String,
        prefix_path: Option<String>,
    ) -> Vec<String> {
        let mut cmd = vec![
            "cmake".to_string(),
            format!("--preset={}", self.preset),
            "-S".to_string(),
            source_dir,
            "-B".to_string(),
            build_dir,
        ];

        if self.ninja {
            cmd.push("-GNinja".to_string());
        }

        if let Some(prefix) = prefix_path {
            cmd.push(format!("-DCMAKE_PREFIX_PATH={}", prefix));
        }

        cmd
    }

    fn build(&self, build_dir: String) -> Vec<String> {
        vec![
            "cmake".to_string(),
            "--build".to_string(),
            build_dir,
            "--parallel".to_string(),
        ]
    }

    pub fn build_target(&self, build_dir: String, target: &str) -> Vec<String> {
        vec![
            "cmake".to_string(),
            "--build".to_string(),
            build_dir,
            "--target".to_string(),
            target.to_string(),
            "--parallel".to_string(),
        ]
    }

    fn install(&self, build_dir: String, install_dir: String) -> Vec<String> {
        vec![
            "cmake".to_string(),
            "--install".to_string(),
            build_dir,
            "--prefix".to_string(),
            install_dir,
        ]
    }

    pub fn clean(&self, paths: &[String]) -> Vec<String> {
        const BASE: [&str; 2] = ["rm", "-rf"];
        let mut cmd = Vec::with_capacity(BASE.len() + paths.len());
        cmd.extend(BASE.iter().map(|s| s.to_string()));
        cmd.extend_from_slice(paths);
        cmd
    }
}
