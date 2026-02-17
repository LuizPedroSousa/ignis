use std::fmt;

use serde::{Deserialize, Serialize};

use crate::Config;

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Deserialize)]
pub enum TargetKind {
    Root,
    Executable,
    Installer,
}

#[derive(Debug, Clone)]
pub struct Target {
    pub path: std::path::PathBuf,
    pub kind: TargetKind,
    pub config: Config,
}

impl fmt::Display for TargetKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            TargetKind::Root => "root",
            TargetKind::Executable => "executable",
            TargetKind::Installer => "installer",
        };

        write!(f, "{value}")
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.path.display(), self.kind)
    }
}
