use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub fn compile_file_patterns(patterns: &[String]) -> Vec<glob::Pattern> {
    patterns
        .iter()
        .filter_map(|p| {
            let file_part = p.rsplit('/').next().unwrap_or(p);
            match glob::Pattern::new(file_part) {
                Ok(pat) => Some(pat),
                Err(e) => {
                    eprintln!("chronicler: invalid file pattern {:?}: {}", p, e);
                    None
                }
            }
        })
        .collect()
}

pub fn is_test_file(path: &Path, compiled: &[glob::Pattern]) -> bool {
    let filename = path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("");
    compiled.iter().any(|p| p.matches(filename))
}

pub fn discover(project_root: &Path, patterns: &[String]) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut results = Vec::new();
    for pattern in patterns {
        let full_pattern = project_root.join(pattern).to_string_lossy().to_string();
        let entries = match glob::glob(&full_pattern) {
            Ok(paths) => paths,
            Err(e) => {
                eprintln!("chronicler: invalid glob pattern {:?}: {}", pattern, e);
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("chronicler: glob error: {}", e);
                    continue;
                }
            };
            let meta = match entry.symlink_metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if !meta.is_file() {
                continue;
            }
            if seen.insert(entry.clone()) {
                results.push(entry);
            }
        }
    }
    results.sort();
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TempDir;
    use std::fs;

    // T-001: patterns: ["**/*.test.ts"] → .test.tsファイル一覧が返る
    #[test]
    fn discovers_ts_test_files() {
        let dir = TempDir::new("discovery-ts");
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src/auth.test.ts"), "test").unwrap();
        fs::write(dir.join("src/cart.test.ts"), "test").unwrap();
        fs::write(dir.join("src/app.ts"), "not a test").unwrap();

        let patterns = vec!["**/*.test.ts".into()];
        let found = discover(&dir, &patterns);

        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|p| p.ends_with("auth.test.ts")));
        assert!(found.iter().any(|p| p.ends_with("cart.test.ts")));
    }

    // T-002: patterns: ["*_test.rs"] → _test.rsファイル一覧が返る
    #[test]
    fn discovers_rust_test_files() {
        let dir = TempDir::new("discovery-rs");
        fs::create_dir_all(dir.join("src/rules")).unwrap();
        fs::write(dir.join("src/rules/eval_test.rs"), "test").unwrap();
        fs::write(dir.join("src/rules/eval.rs"), "not a test").unwrap();

        let patterns = vec!["**/*_test.rs".into()];
        let found = discover(&dir, &patterns);

        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("eval_test.rs"));
    }

    // SEC-001: symlinked test files should be excluded from discovery
    #[test]
    fn skips_symlinked_test_files() {
        let dir = TempDir::new("discovery-symlink");
        let outside = TempDir::new("discovery-symlink-outside");
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src/real.test.ts"), "test").unwrap();

        fs::write(outside.join("evil.test.ts"), "evil test").unwrap();
        std::os::unix::fs::symlink(outside.join("evil.test.ts"), dir.join("src/link.test.ts"))
            .unwrap();

        let patterns = vec!["**/*.test.ts".into()];
        let found = discover(&dir, &patterns);

        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("real.test.ts"));
    }
}
