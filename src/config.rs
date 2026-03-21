use serde::Deserialize;
use std::path::{Path, PathBuf};

pub(crate) const TOOLS_CONFIG_FILE: &str = ".claude/tools.json";

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ConfigSource {
    Default,
    Explicit,
}

#[derive(Debug, PartialEq)]
pub struct ChroniclerConfig {
    pub dir: String,
    pub templates: String,
    pub edit: bool,
    pub stop: bool,
    pub gate: bool,
}

impl Default for ChroniclerConfig {
    fn default() -> Self {
        Self {
            dir: "workspace/docs".into(),
            templates: "workspace/doc-templates".into(),
            edit: true,
            stop: true,
            gate: false,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub enum Layout {
    #[default]
    Centralized,
    Collocated,
}

impl Layout {
    fn parse(s: &str) -> Self {
        match s {
            "collocated" => Self::Collocated,
            "centralized" => Self::Centralized,
            other => {
                eprintln!(
                    "chronicler: unknown layout {:?}, defaulting to centralized",
                    other
                );
                Self::Centralized
            }
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct TestDocsConfig {
    pub enabled: bool,
    pub patterns: Vec<String>,
    pub output: String,
    pub layout: Layout,
    pub dir: String,
    pub language: String,
}

impl Default for TestDocsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            patterns: vec!["**/*.test.ts".into(), "**/*.spec.ts".into()],
            output: "docs/test-reference.md".into(),
            layout: Layout::default(),
            dir: ".test-docs".into(),
            language: "en".into(),
        }
    }
}

impl TestDocsConfig {
    pub fn load(project_dir: &Path) -> Self {
        let (_, td_config, _) = load_both(project_dir);
        td_config
    }

    pub fn yaml_path(&self, project_root: &Path, test_file: &Path) -> PathBuf {
        let relative = test_file.strip_prefix(project_root).unwrap_or(test_file);
        match self.layout {
            Layout::Centralized => {
                let yaml_name = format!("{}.yaml", relative.to_string_lossy());
                project_root.join(&self.dir).join(yaml_name)
            }
            Layout::Collocated => {
                let yaml_name = format!(
                    "{}.testdoc.yaml",
                    test_file.file_name().unwrap_or_default().to_string_lossy()
                );
                test_file.parent().unwrap_or(project_root).join(yaml_name)
            }
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
    templates: Option<String>,
    edit: Option<bool>,
    stop: Option<bool>,
    gate: Option<bool>,
    #[serde(rename = "testDocs")]
    test_docs: Option<TestDocsSection>,
}

#[derive(Deserialize)]
struct TestDocsSection {
    enabled: Option<bool>,
    patterns: Option<Vec<String>>,
    output: Option<String>,
    layout: Option<String>,
    dir: Option<String>,
    language: Option<String>,
}

fn load_tools_json(project_dir: &Path) -> Option<ChroniclerSection> {
    let path = project_dir.join(TOOLS_CONFIG_FILE);
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => {
            eprintln!("chronicler: cannot read {}: {}", path.display(), e);
            return None;
        }
    };
    match serde_json::from_str::<ToolsJson>(&content) {
        Ok(parsed) => parsed.chronicler,
        Err(e) => {
            eprintln!("chronicler: failed to parse {}: {}", path.display(), e);
            None
        }
    }
}

impl ChroniclerConfig {
    pub fn load(project_dir: &Path) -> Self {
        let (config, _, _) = load_both(project_dir);
        config
    }
}

pub fn load_both(project_dir: &Path) -> (ChroniclerConfig, TestDocsConfig, ConfigSource) {
    let Some(section) = load_tools_json(project_dir) else {
        return (
            ChroniclerConfig::default(),
            TestDocsConfig::default(),
            ConfigSource::Default,
        );
    };
    let defaults = ChroniclerConfig::default();
    let td_defaults = TestDocsConfig::default();

    let td_config = match section.test_docs {
        Some(td) => TestDocsConfig {
            enabled: td.enabled.unwrap_or(td_defaults.enabled),
            patterns: td.patterns.unwrap_or(td_defaults.patterns),
            output: td.output.unwrap_or(td_defaults.output),
            layout: td.layout.map(|s| Layout::parse(&s)).unwrap_or_default(),
            dir: td.dir.unwrap_or(td_defaults.dir),
            language: td.language.unwrap_or(td_defaults.language),
        },
        None => td_defaults,
    };

    let config = ChroniclerConfig {
        dir: section.dir.unwrap_or(defaults.dir),
        templates: section.templates.unwrap_or(defaults.templates),
        edit: section.edit.unwrap_or(defaults.edit),
        stop: section.stop.unwrap_or(defaults.stop),
        gate: section.gate.unwrap_or(defaults.gate),
    };

    (config, td_config, ConfigSource::Explicit)
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
            r#"{"chronicler":{"dir":"docs","edit":false,"stop":true}}"#,
        ));
        let config = ChroniclerConfig::load(&dir);
        assert_eq!(
            config,
            ChroniclerConfig {
                dir: "docs".into(),
                templates: "workspace/doc-templates".into(),
                edit: false,
                stop: true,
                gate: false,
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
                templates: "workspace/doc-templates".into(),
                edit: true,
                stop: true,
                gate: false,
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
    fn legacy_mode_field_ignored() {
        let dir = setup_dir(Some(r#"{"chronicler":{"mode":"block","dir":"docs"}}"#));
        let config = ChroniclerConfig::load(&dir);
        assert_eq!(config.dir, "docs");
    }

    #[test]
    fn templates_field_reads_from_config() {
        let dir = setup_dir(Some(r#"{"chronicler":{"templates":"my-templates"}}"#));
        let config = ChroniclerConfig::load(&dir);
        assert_eq!(config.templates, "my-templates");
    }

    // T-017: tools.jsonにtestDocs設定あり → TestDocsConfig反映
    #[test]
    fn test_docs_config_reads_from_tools_json() {
        let dir = setup_dir(Some(
            r#"{"chronicler":{"testDocs":{"enabled":true,"patterns":["**/*_test.rs"],"output":"docs/tests.md","layout":"collocated","dir":"my-docs","language":"ja"}}}"#,
        ));
        let config = TestDocsConfig::load(&dir);
        assert_eq!(
            config,
            TestDocsConfig {
                enabled: true,
                patterns: vec!["**/*_test.rs".into()],
                output: "docs/tests.md".into(),
                layout: Layout::Collocated,
                dir: "my-docs".into(),
                language: "ja".into(),
            }
        );
    }

    // T-018: tools.jsonにtestDocs設定なし → デフォルト値
    #[test]
    fn test_docs_config_defaults_when_missing() {
        let dir = setup_dir(Some(r#"{"chronicler":{"dir":"docs"}}"#));
        let config = TestDocsConfig::load(&dir);
        assert_eq!(config, TestDocsConfig::default());
    }

    // T-019: layout=centralized, dir=".td" → .td/src/rules/eval.rs.yaml
    #[test]
    fn yaml_path_centralized() {
        let config = TestDocsConfig {
            layout: Layout::Centralized,
            dir: ".td".into(),
            ..TestDocsConfig::default()
        };
        let root = Path::new("/project");
        let test_file = Path::new("/project/src/rules/eval.rs");
        let result = config.yaml_path(root, test_file);
        assert_eq!(result, PathBuf::from("/project/.td/src/rules/eval.rs.yaml"));
    }

    // T-020: layout=collocated → src/rules/eval.rs.testdoc.yaml
    #[test]
    fn yaml_path_collocated() {
        let config = TestDocsConfig {
            layout: Layout::Collocated,
            ..TestDocsConfig::default()
        };
        let root = Path::new("/project");
        let test_file = Path::new("/project/src/rules/eval.rs");
        let result = config.yaml_path(root, test_file);
        assert_eq!(
            result,
            PathBuf::from("/project/src/rules/eval.rs.testdoc.yaml")
        );
    }

    #[test]
    fn load_both_explicit_when_chronicler_section_present() {
        let dir = setup_dir(Some(r#"{"chronicler":{}}"#));
        let (_, _, source) = load_both(&dir);
        assert_eq!(source, ConfigSource::Explicit);
    }

    #[test]
    fn load_both_default_when_no_chronicler_section() {
        let dir = setup_dir(Some(r#"{"gates":{"knip":true}}"#));
        let (_, _, source) = load_both(&dir);
        assert_eq!(source, ConfigSource::Default);
    }

    #[test]
    fn load_both_default_when_no_tools_json() {
        let dir = setup_dir(None);
        let (_, _, source) = load_both(&dir);
        assert_eq!(source, ConfigSource::Default);
    }
}
