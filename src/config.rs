use serde::Deserialize;
use std::path::Path;

const TOOLS_CONFIG_FILE: &str = ".claude/tools.json";

#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub enum Mode {
    #[default]
    Warn,
    Block,
}

impl Mode {
    fn parse(s: &str) -> Self {
        match s {
            "block" => Self::Block,
            "warn" => Self::Warn,
            other => {
                eprintln!("chronicler: unknown mode {:?}, defaulting to warn", other);
                Self::Warn
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct ChroniclerConfig {
    pub dir: String,
    pub edit: bool,
    pub stop: bool,
    pub mode: Mode,
}

impl Default for ChroniclerConfig {
    fn default() -> Self {
        Self {
            dir: "workspace/docs".into(),
            edit: true,
            stop: true,
            mode: Mode::default(),
        }
    }
}

#[derive(Deserialize)]
struct ToolsJson {
    chronicler: Option<ChroniclerSection>,
}

#[derive(Deserialize)]
struct ChroniclerSection {
    dir: Option<String>,
    edit: Option<bool>,
    stop: Option<bool>,
    mode: Option<String>,
}

impl ChroniclerConfig {
    pub fn load(project_dir: &Path) -> Self {
        let path = project_dir.join(TOOLS_CONFIG_FILE);
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        let Ok(parsed) = serde_json::from_str::<ToolsJson>(&content) else {
            eprintln!("chronicler: failed to parse {}", path.display());
            return Self::default();
        };
        let Some(section) = parsed.chronicler else {
            return Self::default();
        };
        let defaults = Self::default();
        Self {
            dir: section.dir.unwrap_or(defaults.dir),
            edit: section.edit.unwrap_or(defaults.edit),
            stop: section.stop.unwrap_or(defaults.stop),
            mode: section.mode.map(|s| Mode::parse(&s)).unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TempDir;
    use std::fs;

    fn setup_dir(json: Option<&str>) -> TempDir {
        let dir = TempDir::new("config");
        if let Some(content) = json {
            let claude_dir = dir.join(".claude");
            fs::create_dir_all(&claude_dir).unwrap();
            fs::write(claude_dir.join("tools.json"), content).unwrap();
        }
        dir
    }

    #[test]
    fn reads_chronicler_section() {
        let dir = setup_dir(Some(
            r#"{"chronicler":{"dir":"docs","edit":false,"stop":true,"mode":"block"}}"#,
        ));
        let config = ChroniclerConfig::load(&dir);
        assert_eq!(
            config,
            ChroniclerConfig {
                dir: "docs".into(),
                edit: false,
                stop: true,
                mode: Mode::Block,
            }
        );
    }

    #[test]
    fn missing_file_returns_default() {
        let dir = setup_dir(None);
        let config = ChroniclerConfig::load(&dir);
        assert_eq!(config, ChroniclerConfig::default());
    }

    #[test]
    fn missing_chronicler_section_returns_default() {
        let dir = setup_dir(Some(r#"{"gates":{"knip":true}}"#));
        let config = ChroniclerConfig::load(&dir);
        assert_eq!(config, ChroniclerConfig::default());
    }

    #[test]
    fn partial_section_fills_defaults() {
        let dir = setup_dir(Some(r#"{"chronicler":{"dir":"my-docs"}}"#));
        let config = ChroniclerConfig::load(&dir);
        assert_eq!(
            config,
            ChroniclerConfig {
                dir: "my-docs".into(),
                edit: true,
                stop: true,
                mode: Mode::Warn,
            }
        );
    }

    #[test]
    fn invalid_json_returns_default() {
        let dir = setup_dir(Some("not json{{{"));
        let config = ChroniclerConfig::load(&dir);
        assert_eq!(config, ChroniclerConfig::default());
    }

    #[test]
    fn unknown_mode_defaults_to_warn() {
        let dir = setup_dir(Some(r#"{"chronicler":{"mode":"blokc"}}"#));
        let config = ChroniclerConfig::load(&dir);
        assert_eq!(config.mode, Mode::Warn);
    }
}
