use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct L10n {
    pub en: String,
    pub ja: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TestDocEntry {
    pub hash: String,
    pub approved: Option<String>,
    pub what: L10n,
    pub why: L10n,
    pub test_count: u32,
}

impl Default for TestDocEntry {
    fn default() -> Self {
        Self {
            hash: String::new(),
            approved: None,
            what: L10n {
                en: String::new(),
                ja: String::new(),
            },
            why: L10n {
                en: String::new(),
                ja: String::new(),
            },
            test_count: 0,
        }
    }
}

pub fn read_entry(path: &Path) -> TestDocEntry {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return TestDocEntry::default();
        }
        Err(e) => {
            eprintln!("chronicler: cannot read {}: {}", path.display(), e);
            return TestDocEntry::default();
        }
    };
    match serde_yaml::from_str(&content) {
        Ok(entry) => entry,
        Err(e) => {
            eprintln!(
                "chronicler: invalid test-doc YAML {}: {}",
                path.display(),
                e
            );
            TestDocEntry::default()
        }
    }
}

#[cfg(test)]
pub fn write_entry(path: &Path, entry: &TestDocEntry) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create dir {}: {}", parent.display(), e))?;
    }
    let yaml =
        serde_yaml::to_string(entry).map_err(|e| format!("cannot serialize entry: {}", e))?;
    std::fs::write(path, yaml).map_err(|e| format!("cannot write {}: {}", path.display(), e))?;
    Ok(())
}

#[derive(Debug, PartialEq)]
pub enum EntryStatus {
    Fresh,
    Stale,
    New,
    Orphaned,
}

pub fn check_status(
    yaml_path: &Path,
    test_path: &Path,
    current_hash: &str,
) -> (EntryStatus, Option<TestDocEntry>) {
    let entry = read_entry(yaml_path);

    if entry.hash.is_empty() {
        return (EntryStatus::New, None);
    }

    if !test_path.exists() {
        return (EntryStatus::Orphaned, Some(entry));
    }

    let status = if entry.hash == current_hash {
        EntryStatus::Fresh
    } else {
        EntryStatus::Stale
    };
    (status, Some(entry))
}

#[cfg(test)]
pub fn remove_entry(yaml_path: &Path) -> Result<(), String> {
    if yaml_path.exists() {
        std::fs::remove_file(yaml_path)
            .map_err(|e| format!("cannot remove {}: {}", yaml_path.display(), e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TempDir;
    use std::fs;

    fn sample_entry() -> TestDocEntry {
        TestDocEntry {
            hash: "sha256:abc123".into(),
            approved: Some("2026-03-20".into()),
            what: L10n {
                en: "Detects eval() usage".into(),
                ja: "eval()の使用を検出".into(),
            },
            why: L10n {
                en: "Dynamic code execution enables injection".into(),
                ja: "動的コード実行はインジェクションを可能にする".into(),
            },
            test_count: 8,
        }
    }

    // T-005: 有効なYAMLファイル → TestDocEntry structに変換される
    #[test]
    fn read_valid_yaml() {
        let dir = TempDir::new("read-valid");
        let path = dir.join("eval.rs.yaml");
        let entry = sample_entry();
        let yaml = serde_yaml::to_string(&entry).unwrap();
        fs::write(&path, yaml).unwrap();

        let result = read_entry(&path);
        assert_eq!(result, entry);
    }

    // T-006: 不正なYAMLファイル → warning + 空エントリ
    #[test]
    fn read_invalid_yaml_returns_default() {
        let dir = TempDir::new("read-invalid");
        let path = dir.join("bad.yaml");
        fs::write(&path, "not: [valid: yaml: {{{").unwrap();

        let result = read_entry(&path);
        assert_eq!(result, TestDocEntry::default());
    }

    // T-007: TestDocEntry struct → 有効なYAML出力
    #[test]
    fn write_then_read_roundtrip() {
        let dir = TempDir::new("write-roundtrip");
        let path = dir.join("sub/eval.rs.yaml");
        let entry = sample_entry();

        write_entry(&path, &entry).unwrap();
        assert!(path.exists());

        let result = read_entry(&path);
        assert_eq!(result, entry);
    }

    // T-008: YAMLのhash != 現在hash → staleとして報告
    #[test]
    fn stale_when_hash_differs() {
        let dir = TempDir::new("stale");
        let yaml_path = dir.join("eval.rs.yaml");
        let test_path = dir.join("eval.rs");

        let entry = sample_entry();
        write_entry(&yaml_path, &entry).unwrap();
        fs::write(&test_path, "changed content").unwrap();

        let (status, _) = check_status(&yaml_path, &test_path, "sha256:different");
        assert_eq!(status, EntryStatus::Stale);
    }

    // T-009: YAMLのhash == 現在hash → staleでない
    #[test]
    fn fresh_when_hash_matches() {
        let dir = TempDir::new("fresh");
        let yaml_path = dir.join("eval.rs.yaml");
        let test_path = dir.join("eval.rs");

        let entry = sample_entry();
        write_entry(&yaml_path, &entry).unwrap();
        fs::write(&test_path, "content").unwrap();

        let (status, _) = check_status(&yaml_path, &test_path, "sha256:abc123");
        assert_eq!(status, EntryStatus::Fresh);
    }

    // T-010: テストファイルにYAMLなし → 新規として報告
    #[test]
    fn new_when_no_yaml() {
        let dir = TempDir::new("new");
        let yaml_path = dir.join("eval.rs.yaml");
        let test_path = dir.join("eval.rs");
        fs::write(&test_path, "content").unwrap();

        let (status, _) = check_status(&yaml_path, &test_path, "sha256:xxx");
        assert_eq!(status, EntryStatus::New);
    }

    // T-011: YAMLあり + テストファイル不在 → 孤立エントリとして報告
    #[test]
    fn orphaned_when_test_file_missing() {
        let dir = TempDir::new("orphaned");
        let yaml_path = dir.join("deleted.rs.yaml");
        let test_path = dir.join("deleted.rs");

        let entry = sample_entry();
        write_entry(&yaml_path, &entry).unwrap();

        let (status, _) = check_status(&yaml_path, &test_path, "");
        assert_eq!(status, EntryStatus::Orphaned);
    }

    // T-012: 孤立エントリ削除実行 → YAMLファイルが除去される
    #[test]
    fn remove_entry_deletes_file() {
        let dir = TempDir::new("remove");
        let yaml_path = dir.join("old.rs.yaml");

        let entry = sample_entry();
        write_entry(&yaml_path, &entry).unwrap();
        assert!(yaml_path.exists());

        remove_entry(&yaml_path).unwrap();
        assert!(!yaml_path.exists());
    }
}
