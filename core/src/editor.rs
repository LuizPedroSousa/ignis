use anyhow::{Context, Result};
use arboard::Clipboard;
use std::path::Path;
use std::process::Command;

pub struct Editor {
    editor_command: String,
    vscode_integration: bool,
}

impl Editor {
    pub fn new(config_command: String, vscode_integration: bool) -> Self {
        let editor_command = resolve_editor_command(&config_command);

        Self {
            editor_command,
            vscode_integration,
        }
    }

    pub fn open_file(
        &self,
        file_path: &Path,
        line: Option<usize>,
        column: Option<usize>,
    ) -> Result<()> {
        let editor = &self.editor_command;

        let command =
            if self.vscode_integration && (editor.contains("code") || editor.contains("vscode")) {
                let location = if let Some(line) = line {
                    if let Some(col) = column {
                        format!("{}:{}:{}", file_path.display(), line, col)
                    } else {
                        format!("{}:{}", file_path.display(), line)
                    }
                } else {
                    file_path.display().to_string()
                };
                vec![editor.clone(), "-g".to_string(), location]
            } else if editor.contains("vim") || editor.contains("nvim") {
                let mut cmd = vec![editor.clone()];
                if let Some(line) = line {
                    cmd.push(format!("+{}", line));
                }
                cmd.push(file_path.display().to_string());
                cmd
            } else if editor.contains("emacs") {
                let location = if let Some(line) = line {
                    format!("+{}:{}", line, column.unwrap_or(0))
                } else {
                    file_path.display().to_string()
                };
                vec![editor.clone(), location]
            } else {
                vec![editor.clone(), file_path.display().to_string()]
            };

        Command::new(&command[0])
            .args(&command[1..])
            .spawn()
            .context("Failed to spawn editor")?;

        Ok(())
    }
}

pub fn copy_to_clipboard(text: &str) -> Result<()> {
    let mut clipboard = Clipboard::new().context("Failed to access clipboard")?;
    clipboard
        .set_text(text.to_string())
        .context("Failed to set clipboard text")?;
    Ok(())
}

fn resolve_editor_command(config_command: &str) -> String {
    if config_command == "${EDITOR}" {
        if let Ok(editor) = std::env::var("EDITOR") {
            return editor;
        }
    } else {
        return config_command.to_string();
    }

    let fallbacks = ["code", "vim", "nvim", "emacs", "nano", "gedit"];

    for editor in &fallbacks {
        if Command::new("which")
            .arg(editor)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return editor.to_string();
        }
    }

    "vim".to_string()
}
