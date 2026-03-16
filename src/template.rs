use std::path::{Path, PathBuf};

pub const TEMPLATE_NAMES: &[&str] = &["architecture", "api", "domain", "setup"];

const DEFAULT_ARCHITECTURE: &str = include_str!("templates/architecture.md");
const DEFAULT_API: &str = include_str!("templates/api.md");
const DEFAULT_DOMAIN: &str = include_str!("templates/domain.md");
const DEFAULT_SETUP: &str = include_str!("templates/setup.md");

fn default_content(name: &str) -> &'static str {
    match name {
        "architecture" => DEFAULT_ARCHITECTURE,
        "api" => DEFAULT_API,
        "domain" => DEFAULT_DOMAIN,
        "setup" => DEFAULT_SETUP,
        _ => "",
    }
}

/// Returns true if any files were written, false if all already existed.
pub fn write_defaults(templates_dir: &Path) -> bool {
    if let Err(e) = std::fs::create_dir_all(templates_dir) {
        eprintln!("chronicler: failed to create templates dir: {}", e);
        return false;
    }
    let mut wrote_any = false;
    for name in TEMPLATE_NAMES {
        let path = templates_dir.join(format!("{}.md", name));
        if path.is_file() {
            continue;
        }
        let content = default_content(name);
        if let Err(e) = std::fs::write(&path, content) {
            eprintln!("chronicler: failed to write template {}: {}", name, e);
        } else {
            wrote_any = true;
        }
    }
    wrote_any
}

pub fn list_template_paths(templates_dir: &Path) -> Vec<PathBuf> {
    if !templates_dir.is_dir() {
        return Vec::new();
    }
    let canonical = templates_dir.canonicalize().unwrap_or(templates_dir.to_path_buf());
    let mut paths: Vec<PathBuf> = TEMPLATE_NAMES
        .iter()
        .map(|name| canonical.join(format!("{}.md", name)))
        .filter(|p| p.is_file())
        .collect();
    paths.sort();
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TempDir;

    // T-001: all 4 template constants are non-empty
    #[test]
    fn t_001_template_constants_are_non_empty() {
        for name in TEMPLATE_NAMES {
            let content = default_content(name);
            assert!(!content.is_empty(), "template {} should not be empty", name);
        }
        assert_eq!(TEMPLATE_NAMES.len(), 4);
    }

    // T-002: write_defaults creates template files
    #[test]
    fn t_002_write_defaults_creates_files() {
        let tmp = TempDir::new("template-write");
        let dir = tmp.join("templates");

        let result = write_defaults(&dir);
        assert!(result, "should return true when writing");
        assert!(dir.is_dir(), "directory should exist");

        for name in TEMPLATE_NAMES {
            let path = dir.join(format!("{}.md", name));
            assert!(path.is_file(), "{}.md should exist", name);
            let content = std::fs::read_to_string(&path).unwrap();
            assert!(!content.is_empty(), "{}.md should not be empty", name);
        }
    }

    // T-003: write_defaults skips existing files, fills missing ones
    #[test]
    fn t_003_write_defaults_fills_missing() {
        let tmp = TempDir::new("template-partial");
        let dir = tmp.join("templates");
        std::fs::create_dir_all(&dir).unwrap();
        // User has custom architecture.md
        std::fs::write(dir.join("architecture.md"), "custom arch").unwrap();

        let result = write_defaults(&dir);
        assert!(result, "should return true when filling missing");
        // custom architecture.md preserved
        assert_eq!(
            std::fs::read_to_string(dir.join("architecture.md")).unwrap(),
            "custom arch"
        );
        // missing templates filled
        assert!(dir.join("api.md").is_file());
        assert!(dir.join("domain.md").is_file());
        assert!(dir.join("setup.md").is_file());
    }

    // T-003b: write_defaults returns false when all exist
    #[test]
    fn t_003b_write_defaults_all_exist() {
        let tmp = TempDir::new("template-allexist");
        let dir = tmp.join("templates");
        write_defaults(&dir);

        let result = write_defaults(&dir);
        assert!(!result, "should return false when all exist");
    }

    // T-004: list_template_paths returns paths
    #[test]
    fn t_004_list_template_paths() {
        let tmp = TempDir::new("template-list");
        let dir = tmp.join("templates");
        write_defaults(&dir);

        let paths = list_template_paths(&dir);
        assert_eq!(paths.len(), 4, "should return 4 paths");
        let names: Vec<String> = paths
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(names.contains(&"architecture.md".to_string()));
        assert!(names.contains(&"api.md".to_string()));
        assert!(names.contains(&"domain.md".to_string()));
        assert!(names.contains(&"setup.md".to_string()));
    }

    #[test]
    fn list_template_paths_empty_dir() {
        let tmp = TempDir::new("template-empty");
        let paths = list_template_paths(&tmp.join("nonexistent"));
        assert!(paths.is_empty());
    }
}
