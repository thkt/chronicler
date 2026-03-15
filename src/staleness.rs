use crate::scanner::DocRefs;
use std::collections::HashSet;
use std::path::Path;

pub struct StaleDoc {
    pub doc_relative: String,
    pub stale_files: Vec<String>,
}

pub fn check_staleness(project_root: &Path, docs: &[DocRefs]) -> Vec<StaleDoc> {
    let mut stale_docs = Vec::new();

    for doc in docs {
        let doc_mtime = match doc.doc_path.metadata().and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => continue,
        };

        let unique_files: HashSet<&str> = doc.file_refs.iter().map(|s| s.as_str()).collect();

        let mut stale_files: Vec<String> = unique_files
            .into_iter()
            .filter(|file_ref| {
                let abs_path = project_root.join(file_ref);
                match abs_path.metadata().and_then(|m| m.modified()) {
                    Ok(ref_mtime) => ref_mtime > doc_mtime,
                    Err(_) => false,
                }
            })
            .map(String::from)
            .collect();

        if !stale_files.is_empty() {
            stale_files.sort();
            let doc_relative = doc
                .doc_path
                .strip_prefix(project_root)
                .unwrap_or(&doc.doc_path)
                .to_string_lossy()
                .to_string();
            stale_docs.push(StaleDoc {
                doc_relative,
                stale_files,
            });
        }
    }

    stale_docs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{self, TempDir};
    use std::fs;

    fn setup_project(doc_content: &str) -> (TempDir, Vec<DocRefs>) {
        let tmp = TempDir::new("staleness");
        fs::create_dir_all(tmp.join(".git")).unwrap();
        fs::create_dir_all(tmp.join("docs")).unwrap();
        fs::create_dir_all(tmp.join("src")).unwrap();

        let doc_path = tmp.join("docs/arch.md");
        fs::write(&doc_path, doc_content).unwrap();

        let docs = vec![DocRefs {
            doc_path,
            file_refs: vec!["src/auth.ts".into()],
        }];

        (tmp, docs)
    }

    #[test]
    fn detects_stale_doc() {
        let (tmp, docs) = setup_project("See src/auth.ts:1");
        test_utils::set_mtime_past(&tmp.join("docs/arch.md"), 3600);
        fs::write(tmp.join("src/auth.ts"), "content").unwrap();

        let stale = check_staleness(&tmp, &docs);
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].doc_relative, "docs/arch.md");
        assert_eq!(stale[0].stale_files, vec!["src/auth.ts"]);
    }

    #[test]
    fn fresh_doc_not_reported() {
        let (tmp, docs) = setup_project("See src/auth.ts:1");
        fs::write(tmp.join("src/auth.ts"), "content").unwrap();
        test_utils::set_mtime_past(&tmp.join("src/auth.ts"), 3600);
        fs::write(tmp.join("docs/arch.md"), "See src/auth.ts:1").unwrap();

        let stale = check_staleness(&tmp, &docs);
        assert!(stale.is_empty());
    }

    #[test]
    fn missing_source_not_stale() {
        let (tmp, docs) = setup_project("See src/auth.ts:1");

        let stale = check_staleness(&tmp, &docs);
        assert!(stale.is_empty());
    }

    #[test]
    fn equal_mtime_not_stale() {
        let (tmp, docs) = setup_project("See src/auth.ts:1");
        fs::write(tmp.join("src/auth.ts"), "content").unwrap();

        let time = std::time::SystemTime::now();
        test_utils::set_mtime(&tmp.join("docs/arch.md"), time);
        test_utils::set_mtime(&tmp.join("src/auth.ts"), time);

        let stale = check_staleness(&tmp, &docs);
        assert!(stale.is_empty());
    }

    #[test]
    fn multiple_stale_files() {
        let tmp = TempDir::new("staleness-multi");
        fs::create_dir_all(tmp.join(".git")).unwrap();
        fs::create_dir_all(tmp.join("docs")).unwrap();
        fs::create_dir_all(tmp.join("src")).unwrap();

        let doc_path = tmp.join("docs/arch.md");
        fs::write(&doc_path, "See src/a.ts:1 and src/b.ts:2").unwrap();
        test_utils::set_mtime_past(&doc_path, 3600);

        fs::write(tmp.join("src/a.ts"), "a").unwrap();
        fs::write(tmp.join("src/b.ts"), "b").unwrap();

        let docs = vec![DocRefs {
            doc_path,
            file_refs: vec!["src/a.ts".into(), "src/b.ts".into()],
        }];

        let stale = check_staleness(&tmp, &docs);
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0].stale_files, vec!["src/a.ts", "src/b.ts"]);
    }
}
