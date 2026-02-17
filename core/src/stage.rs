use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Stage {
    PreValidation,
    Configure,
    Build,
    Install,
    Clean,
    PostBuild,
    Test,
    Exec,
}

#[derive(Debug, Clone)]
pub struct StageMetadata {
    pub description: String,
    pub can_run_concurrent: bool,
    pub is_optional: bool,
}

impl Stage {
    pub fn metadata(&self) -> StageMetadata {
        match self {
            Stage::PreValidation => StageMetadata {
                description: "Pre-validation checks".to_string(),
                can_run_concurrent: false,
                is_optional: true,
            },
            Stage::Configure => StageMetadata {
                description: "CMake configuration".to_string(),
                can_run_concurrent: false,
                is_optional: false,
            },
            Stage::Build => StageMetadata {
                description: "Building project".to_string(),
                can_run_concurrent: false,
                is_optional: false,
            },
            Stage::Install => StageMetadata {
                description: "Installing artifacts".to_string(),
                can_run_concurrent: true,
                is_optional: false,
            },
            Stage::Clean => StageMetadata {
                description: "Cleaning build artifacts".to_string(),
                can_run_concurrent: false,
                is_optional: false,
            },
            Stage::PostBuild => StageMetadata {
                description: "Post-build processing".to_string(),
                can_run_concurrent: true,
                is_optional: true,
            },
            Stage::Test => StageMetadata {
                description: "Running tests".to_string(),
                can_run_concurrent: true,
                is_optional: true,
            },
            Stage::Exec => StageMetadata {
                description: "Executing program".to_string(),
                can_run_concurrent: false,
                is_optional: false,
            },
        }
    }

    pub fn default_dependencies(&self) -> Vec<Stage> {
        match self {
            Stage::PreValidation => vec![],
            Stage::Configure => vec![Stage::PreValidation],
            Stage::Build => vec![Stage::Configure],
            Stage::Install => vec![Stage::Build],
            Stage::PostBuild => vec![Stage::Build],
            Stage::Test => vec![Stage::Build],
            Stage::Exec => vec![Stage::Build, Stage::Install],
            Stage::Clean => vec![],
        }
    }

    pub fn all() -> Vec<Stage> {
        vec![
            Stage::PreValidation,
            Stage::Configure,
            Stage::Build,
            Stage::Install,
            Stage::Clean,
            Stage::PostBuild,
            Stage::Test,
            Stage::Exec,
        ]
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Stage::PreValidation => "PreValidation",
            Stage::Configure => "Configure",
            Stage::Build => "Build",
            Stage::Install => "Install",
            Stage::Clean => "Clean",
            Stage::PostBuild => "PostBuild",
            Stage::Test => "Test",
            Stage::Exec => "Exec",
        };
        write!(f, "{}", name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stage_metadata() {
        let stage = Stage::Build;
        let metadata = stage.metadata();
        assert_eq!(metadata.description, "Building project");
        assert!(!metadata.can_run_concurrent);
        assert!(!metadata.is_optional);
    }

    #[test]
    fn test_stage_dependencies() {
        assert_eq!(Stage::PreValidation.default_dependencies(), vec![]);
        assert_eq!(Stage::Configure.default_dependencies(), vec![Stage::PreValidation]);
        assert_eq!(Stage::Build.default_dependencies(), vec![Stage::Configure]);
        assert_eq!(Stage::Install.default_dependencies(), vec![Stage::Build]);
        assert_eq!(Stage::Exec.default_dependencies(), vec![Stage::Build, Stage::Install]);
    }

    #[test]
    fn test_stage_display() {
        assert_eq!(format!("{}", Stage::Build), "Build");
        assert_eq!(format!("{}", Stage::Configure), "Configure");
    }
}
