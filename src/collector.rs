use std::path::Path;

const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    ".claude",
    "__pycache__",
    ".next",
    "dist",
    "build",
    ".turbo",
];

pub struct SourceTree {
    pub entries: Vec<TreeEntry>,
}

pub struct TreeEntry {
    pub path: String,
    pub is_dir: bool,
}

pub fn collect_tree(project_root: &Path) -> SourceTree {
    let mut entries = Vec::new();
    walk_tree(project_root, project_root, &mut entries);
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    SourceTree { entries }
}

fn walk_tree(root: &Path, dir: &Path, entries: &mut Vec<TreeEntry>) {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return;
    };
    let rel_path = |p: &Path| crate::relative_path(p, root);
    for entry in read_dir.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let path = entry.path();
        if ft.is_dir() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if SKIP_DIRS.contains(&name_str.as_ref()) {
                continue;
            }
            entries.push(TreeEntry {
                path: rel_path(&path),
                is_dir: true,
            });
            walk_tree(root, &path, entries);
        } else {
            entries.push(TreeEntry {
                path: rel_path(&path),
                is_dir: false,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TempDir;
    use std::fs;

    /// [T-001] when project has src/ and lib/ directories, should include all source files
    #[test]
    fn t_001_collects_source_files_from_src_and_lib() {
        let tmp = TempDir::new("collector-t001");
        fs::create_dir_all(tmp.join(".git")).unwrap();
        fs::create_dir_all(tmp.join("src")).unwrap();
        fs::create_dir_all(tmp.join("lib")).unwrap();
        fs::write(tmp.join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(tmp.join("src/lib.rs"), "pub mod foo;").unwrap();
        fs::write(tmp.join("lib/utils.rs"), "pub fn foo() {}").unwrap();
        fs::write(tmp.join("Cargo.toml"), "[package]").unwrap();

        let tree = collect_tree(&tmp);

        let file_paths: Vec<&str> = tree
            .entries
            .iter()
            .filter(|e| !e.is_dir)
            .map(|e| e.path.as_str())
            .collect();
        assert!(
            file_paths.contains(&"src/main.rs"),
            "expected src/main.rs in {file_paths:?}"
        );
        assert!(
            file_paths.contains(&"src/lib.rs"),
            "expected src/lib.rs in {file_paths:?}"
        );
        assert!(
            file_paths.contains(&"lib/utils.rs"),
            "expected lib/utils.rs in {file_paths:?}"
        );
        assert!(
            file_paths.contains(&"Cargo.toml"),
            "expected Cargo.toml in {file_paths:?}"
        );

        let dir_paths: Vec<&str> = tree
            .entries
            .iter()
            .filter(|e| e.is_dir)
            .map(|e| e.path.as_str())
            .collect();
        assert!(
            dir_paths.contains(&"src"),
            "expected src dir in {dir_paths:?}"
        );
        assert!(
            dir_paths.contains(&"lib"),
            "expected lib dir in {dir_paths:?}"
        );
    }

    /// [T-002] when project has .git, node_modules, target, should skip them
    #[test]
    fn t_002_skips_excluded_directories() {
        let tmp = TempDir::new("collector-t002");
        fs::create_dir_all(tmp.join(".git/objects")).unwrap();
        fs::create_dir_all(tmp.join("node_modules/pkg")).unwrap();
        fs::create_dir_all(tmp.join("target/debug")).unwrap();
        fs::create_dir_all(tmp.join("src")).unwrap();
        fs::write(tmp.join("node_modules/pkg/index.js"), "x").unwrap();
        fs::write(tmp.join("target/debug/bin"), "x").unwrap();
        fs::write(tmp.join("src/lib.rs"), "").unwrap();

        let tree = collect_tree(&tmp);
        let paths: Vec<&str> = tree.entries.iter().map(|e| e.path.as_str()).collect();

        assert!(
            paths.contains(&"src/lib.rs"),
            "expected src/lib.rs in {paths:?}"
        );
        assert!(
            !paths.iter().any(|p| p.starts_with(".git")),
            ".git should be excluded but found in {paths:?}"
        );
        assert!(
            !paths.iter().any(|p| p.starts_with("node_modules")),
            "node_modules should be excluded but found in {paths:?}"
        );
        assert!(
            !paths.iter().any(|p| p.starts_with("target")),
            "target should be excluded but found in {paths:?}"
        );
    }

    /// [T-003] when project has no files (just .git), should return empty SourceTree
    #[test]
    fn t_003_empty_project_returns_empty_tree() {
        let tmp = TempDir::new("collector-t003");
        fs::create_dir_all(tmp.join(".git")).unwrap();

        let tree = collect_tree(&tmp);

        assert!(
            tree.entries.is_empty(),
            "expected empty entries but got {} items",
            tree.entries.len()
        );
    }

    /// [T-004] when project has symlinks, should skip them
    #[test]
    fn t_004_skips_symlinked_files_and_directories() {
        let tmp = TempDir::new("collector-t004");
        let outside = TempDir::new("collector-t004-outside");
        fs::create_dir_all(tmp.join(".git")).unwrap();
        fs::create_dir_all(tmp.join("src")).unwrap();
        fs::write(tmp.join("src/real.rs"), "fn real() {}").unwrap();

        // Symlinked file (target outside project root)
        fs::write(outside.join("secret.rs"), "fn secret() {}").unwrap();
        std::os::unix::fs::symlink(outside.join("secret.rs"), tmp.join("src/link.rs")).unwrap();

        // Symlinked directory (target outside project root)
        let outside_sub = outside.join("subdir");
        fs::create_dir_all(&outside_sub).unwrap();
        fs::write(outside_sub.join("hidden.rs"), "fn hidden() {}").unwrap();
        std::os::unix::fs::symlink(&outside_sub, tmp.join("src/linked_dir")).unwrap();

        let tree = collect_tree(&tmp);
        let paths: Vec<&str> = tree.entries.iter().map(|e| e.path.as_str()).collect();

        assert!(
            paths.contains(&"src/real.rs"),
            "expected src/real.rs in {paths:?}"
        );
        assert!(
            !paths.iter().any(|p| p.contains("link.rs")),
            "symlinked file should be excluded: {paths:?}"
        );
        assert!(
            !paths.iter().any(|p| p.contains("linked_dir")),
            "symlinked directory should be excluded: {paths:?}"
        );
        assert!(
            !paths.iter().any(|p| p.contains("hidden.rs")),
            "files inside symlinked dir should be excluded: {paths:?}"
        );
    }

    /// [NFR-003] when project_root does not exist, should return empty tree without panic
    #[test]
    fn nonexistent_root_returns_empty_tree() {
        let tree = collect_tree(Path::new("/nonexistent/path/that/does/not/exist"));
        assert!(tree.entries.is_empty());
    }
}
