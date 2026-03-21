use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

static REF_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|[\s\[`])([a-zA-Z0-9_./-]+\.[a-zA-Z0-9]+):(\d+)").unwrap());

pub struct DocRefs {
    pub doc_path: PathBuf,
    pub file_refs: Vec<String>,
}

const MAX_FILE_SIZE: u64 = 1_048_576;

pub(crate) fn extract_refs(content: &str) -> Vec<String> {
    REF_RE
        .captures_iter(content)
        .map(|cap| cap[1].to_string())
        .collect()
}

pub fn scan_docs(docs_dir: &Path) -> Vec<DocRefs> {
    let mut results = Vec::new();
    if !docs_dir.is_dir() {
        return results;
    }
    walk_md_files(docs_dir, &mut |md_path| {
        let mut file = match std::fs::File::open(md_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("chronicler: skipping {}: {}", md_path.display(), e);
                return;
            }
        };
        if let Ok(meta) = file.metadata()
            && meta.len() > MAX_FILE_SIZE
        {
            eprintln!(
                "chronicler: skipping {} (exceeds size limit)",
                md_path.display()
            );
            return;
        }
        let mut content = String::new();
        if std::io::Read::read_to_string(&mut file, &mut content).is_err() {
            eprintln!("chronicler: failed to read {}", md_path.display());
            return;
        };
        let file_refs = extract_refs(&content);
        if !file_refs.is_empty() {
            results.push(DocRefs {
                doc_path: md_path.to_path_buf(),
                file_refs,
            });
        }
    });
    results.sort_by(|a, b| a.doc_path.cmp(&b.doc_path));
    results
}

pub fn find_refs_to_file<'a>(docs: &'a [DocRefs], target_relative: &str) -> Vec<(&'a Path, usize)> {
    let target_basename = Path::new(target_relative)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    let basename_unique = is_basename_unique(docs, target_basename);

    docs.iter()
        .filter_map(|doc| {
            let count = doc
                .file_refs
                .iter()
                .filter(|r| is_match(r, target_relative, target_basename, basename_unique))
                .count();
            (count > 0).then_some((&*doc.doc_path, count))
        })
        .collect()
}

fn is_match(
    reference: &str,
    target_relative: &str,
    target_basename: &str,
    basename_unique: bool,
) -> bool {
    if reference == target_relative {
        return true;
    }
    if basename_unique {
        let ref_basename = Path::new(reference)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        return ref_basename == target_basename;
    }
    false
}

fn is_basename_unique(docs: &[DocRefs], basename: &str) -> bool {
    let mut paths_with_basename: HashSet<&str> = HashSet::new();
    for doc in docs {
        for r in &doc.file_refs {
            let ref_basename = Path::new(r.as_str())
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if ref_basename == basename {
                paths_with_basename.insert(r.as_str());
                if paths_with_basename.len() > 1 {
                    return false;
                }
            }
        }
    }
    paths_with_basename.len() == 1
}

pub(crate) fn walk_files_by_ext(dir: &Path, ext: &str, visitor: &mut impl FnMut(&Path)) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => {
            eprintln!("chronicler: cannot read directory {}: {}", dir.display(), e);
            return;
        }
    };
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_symlink() {
            continue;
        }
        let path = entry.path();
        if ft.is_dir() {
            walk_files_by_ext(&path, ext, visitor);
        } else if path.extension().is_some_and(|e| e == ext) {
            visitor(&path);
        }
    }
}

fn walk_md_files(dir: &Path, visitor: &mut impl FnMut(&Path)) {
    walk_files_by_ext(dir, "md", visitor);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TempDir;
    use std::fs;

    fn setup_docs(files: &[(&str, &str)]) -> TempDir {
        let tmp = TempDir::new("scanner");
        let docs = tmp.join("docs");
        fs::create_dir_all(&docs).unwrap();
        for (name, content) in files {
            let path = docs.join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, content).unwrap();
        }
        tmp
    }

    #[test]
    fn extract_refs_basic() {
        let refs = extract_refs("See src/auth.ts:42 for details");
        assert_eq!(refs, vec!["src/auth.ts"]);
    }

    #[test]
    fn extract_refs_multiple_formats() {
        let content = "Plain src/a.ts:1\nBacktick `src/b.ts:2`\nBracket [src/c.ts:3]";
        let refs = extract_refs(content);
        assert_eq!(refs, vec!["src/a.ts", "src/b.ts", "src/c.ts"]);
    }

    #[test]
    fn extract_refs_no_matches() {
        assert!(extract_refs("No references here").is_empty());
    }

    #[test]
    fn extract_refs_duplicate_paths() {
        let refs = extract_refs("src/a.ts:1 and src/a.ts:2");
        assert_eq!(refs, vec!["src/a.ts", "src/a.ts"]);
    }

    #[test]
    fn scan_docs_collects_refs_from_files() {
        let tmp = setup_docs(&[("arch.md", "See src/utils/auth.ts:42 for details")]);
        let docs = scan_docs(&tmp.join("docs"));
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].file_refs, vec!["src/utils/auth.ts"]);
    }

    #[test]
    fn skips_files_without_references() {
        let tmp = setup_docs(&[("readme.md", "No code references here")]);
        let docs = scan_docs(&tmp.join("docs"));
        assert!(docs.is_empty());
    }

    #[test]
    fn handles_nonexistent_dir() {
        let tmp = TempDir::new("scanner-none");
        let docs = scan_docs(&tmp.join("nonexistent"));
        assert!(docs.is_empty());
    }

    #[test]
    fn scans_nested_directories() {
        let tmp = setup_docs(&[
            ("api/endpoints.md", "See src/api.ts:1"),
            ("guides/setup.md", "See src/config.ts:5"),
        ]);
        let docs = scan_docs(&tmp.join("docs"));
        assert_eq!(docs.len(), 2);
    }

    #[test]
    fn find_refs_exact_path_match() {
        let docs = vec![DocRefs {
            doc_path: PathBuf::from("/project/docs/arch.md"),
            file_refs: vec!["src/auth.ts".into(), "src/db.ts".into()],
        }];
        let matches = find_refs_to_file(&docs, "src/auth.ts");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].1, 1);
    }

    #[test]
    fn find_refs_counts_duplicates() {
        let docs = vec![DocRefs {
            doc_path: PathBuf::from("/project/docs/arch.md"),
            file_refs: vec![
                "src/auth.ts".into(),
                "src/db.ts".into(),
                "src/auth.ts".into(),
            ],
        }];
        let matches = find_refs_to_file(&docs, "src/auth.ts");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].1, 2);
    }

    #[test]
    fn find_refs_basename_match_when_unique() {
        let docs = vec![DocRefs {
            doc_path: PathBuf::from("/project/docs/arch.md"),
            file_refs: vec!["src/utils/auth.ts".into()],
        }];
        let matches = find_refs_to_file(&docs, "src/auth.ts");
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn find_refs_no_basename_match_when_ambiguous() {
        let docs = vec![DocRefs {
            doc_path: PathBuf::from("/project/docs/arch.md"),
            file_refs: vec!["src/utils/auth.ts".into(), "lib/auth.ts".into()],
        }];
        let matches = find_refs_to_file(&docs, "pkg/auth.ts");
        assert!(matches.is_empty());
    }

    #[test]
    fn find_refs_no_match() {
        let docs = vec![DocRefs {
            doc_path: PathBuf::from("/project/docs/arch.md"),
            file_refs: vec!["src/db.ts".into()],
        }];
        let matches = find_refs_to_file(&docs, "src/auth.ts");
        assert!(matches.is_empty());
    }

    #[test]
    fn reference_at_line_start() {
        let tmp = setup_docs(&[("arch.md", "src/foo.ts:1\nMore text")]);
        let docs = scan_docs(&tmp.join("docs"));
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].file_refs, vec!["src/foo.ts"]);
    }

    #[test]
    fn skips_symlinks() {
        let tmp = TempDir::new("scanner-symlink");
        let docs = tmp.join("docs");
        fs::create_dir_all(&docs).unwrap();
        fs::write(docs.join("real.md"), "See src/foo.ts:1").unwrap();

        let outside = tmp.join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("evil.md"), "See src/secret.ts:1").unwrap();
        std::os::unix::fs::symlink(outside.join("evil.md"), docs.join("link.md")).unwrap();

        let results = scan_docs(&docs);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_refs, vec!["src/foo.ts"]);
    }

    #[test]
    fn skips_symlinked_directories() {
        let tmp = TempDir::new("scanner-symdir");
        let docs = tmp.join("docs");
        fs::create_dir_all(&docs).unwrap();
        fs::write(docs.join("real.md"), "See src/foo.ts:1").unwrap();

        let outside = tmp.join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("evil.md"), "See src/secret.ts:1").unwrap();
        std::os::unix::fs::symlink(&outside, docs.join("linked_dir")).unwrap();

        let results = scan_docs(&docs);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn skips_oversized_files() {
        let tmp = TempDir::new("scanner-big");
        let docs = tmp.join("docs");
        fs::create_dir_all(&docs).unwrap();
        fs::write(docs.join("small.md"), "See src/foo.ts:1").unwrap();

        let big_content = "x".repeat(MAX_FILE_SIZE as usize + 1);
        fs::write(docs.join("huge.md"), big_content).unwrap();

        let results = scan_docs(&docs);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].file_refs, vec!["src/foo.ts"]);
    }
}
