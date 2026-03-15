mod config;
mod sanitize;
mod scanner;
mod staleness;
#[cfg(test)]
mod test_utils;
mod traverse;

use std::io::{IsTerminal, Read};
use std::path::Path;

const MAX_CONTEXT_LINES: usize = 100;

fn run_edit(input: &str) -> Option<String> {
    let json: serde_json::Value = match serde_json::from_str(input) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("chronicler: failed to parse hook input: {}", e);
            return None;
        }
    };
    let file_path_str = json["tool_input"]["file_path"].as_str()?;

    if file_path_str.ends_with(".md") {
        return None;
    }

    let file_path = Path::new(file_path_str);
    let start = file_path.parent().unwrap_or(file_path);
    let project_root = traverse::find_project_root(start)?;

    let config = config::ChroniclerConfig::load(project_root);
    if !config.edit {
        return None;
    }

    let docs_dir = project_root.join(&config.dir);
    let docs = scanner::scan_docs(&docs_dir);
    if docs.is_empty() {
        return None;
    }

    let target_relative = file_path
        .strip_prefix(project_root)
        .ok()?
        .to_string_lossy();

    let matches = scanner::find_refs_to_file(&docs, &target_relative);
    if matches.is_empty() {
        return None;
    }

    let doc_lines: Vec<String> = matches
        .iter()
        .map(|(doc_path, count)| {
            let doc_rel = doc_path
                .strip_prefix(project_root)
                .unwrap_or(doc_path)
                .to_string_lossy();
            let ref_word = if *count == 1 {
                "reference"
            } else {
                "references"
            };
            format!(
                "- {} ({} {} to {})",
                doc_rel, count, ref_word, target_relative
            )
        })
        .collect();

    let body = sanitize::tail_lines(&doc_lines.join("\n"), MAX_CONTEXT_LINES);
    let context = format!(
        "## chronicler\n\nThe following docs may need updating:\n{}\n\nRun `/docs` to regenerate.",
        body
    );

    let output = serde_json::json!({
        "decision": "approve",
        "reason": "chronicler: edited file is referenced in documentation",
        "additionalContext": context
    });

    Some(output.to_string())
}

fn run_stop(project_dir: &Path) -> Option<String> {
    let project_root = traverse::find_project_root(project_dir)?;
    let config = config::ChroniclerConfig::load(project_root);
    if !config.stop {
        return None;
    }

    let docs_dir = project_root.join(&config.dir);
    let docs = scanner::scan_docs(&docs_dir);
    if docs.is_empty() {
        return None;
    }

    let stale = staleness::check_staleness(project_root, &docs);
    if stale.is_empty() {
        return None;
    }

    Some(build_stop_output(&stale, config.mode).to_string())
}

fn build_stop_output(stale: &[staleness::StaleDoc], mode: config::Mode) -> serde_json::Value {
    match mode {
        config::Mode::Block => {
            let sections: Vec<String> = stale
                .iter()
                .map(|s| {
                    let files = s.stale_files.join(", ");
                    format!("## {}\n{} modified after doc generation", s.doc_relative, files)
                })
                .collect();
            let body = sanitize::tail_lines(&sections.join("\n\n"), MAX_CONTEXT_LINES);
            let reason = format!(
                "chronicler: {} {} outdated.\n\n{}\n\nRun `/docs` to update.",
                stale.len(),
                if stale.len() == 1 { "document is" } else { "documents are" },
                body
            );
            serde_json::json!({ "decision": "block", "reason": reason })
        }
        config::Mode::Warn => {
            let lines: Vec<String> = stale
                .iter()
                .map(|s| {
                    let files = s.stale_files.join(", ");
                    format!("- {} ({} modified after doc generation)", s.doc_relative, files)
                })
                .collect();
            let body = sanitize::tail_lines(&lines.join("\n"), MAX_CONTEXT_LINES);
            let context = format!(
                "## chronicler\n\nStale documentation detected:\n{}\n\nRun `/docs` to update.",
                body
            );
            serde_json::json!({
                "decision": "approve",
                "reason": "chronicler: documentation may be outdated",
                "additionalContext": context
            })
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() >= 2 {
        let dir = &args[1];
        let project_dir = Path::new(dir);
        if !project_dir.is_dir() {
            eprintln!("chronicler: not a directory: {}", project_dir.display());
            std::process::exit(1);
        }
        if let Some(json) = run_stop(project_dir) {
            println!("{}", json);
        }
        return;
    }

    if std::io::stdin().is_terminal() {
        if let Some(json) = run_stop(Path::new(".")) {
            println!("{}", json);
        }
        return;
    }

    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("chronicler: failed to read stdin: {}", e);
        return;
    }

    if let Some(json) = run_edit(&input) {
        println!("{}", json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use test_utils::TempDir;

    fn setup_project(config_json: &str, docs: &[(&str, &str)], sources: &[&str]) -> TempDir {
        let tmp = TempDir::new("main");
        fs::create_dir_all(tmp.join(".git")).unwrap();
        fs::create_dir_all(tmp.join(".claude")).unwrap();
        fs::write(tmp.join(".claude/tools.json"), config_json).unwrap();

        let doc_dir = tmp.join("workspace/docs");
        fs::create_dir_all(&doc_dir).unwrap();
        for (name, content) in docs {
            let path = doc_dir.join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, content).unwrap();
        }

        for src in sources {
            let path = tmp.join(src);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(path, "content").unwrap();
        }

        tmp
    }

    #[test]
    fn edit_returns_advisory_when_refs_found() {
        let tmp = setup_project(
            r#"{"chronicler":{}}"#,
            &[("arch.md", "See src/auth.ts:42 for auth logic")],
            &["src/auth.ts"],
        );

        let input = serde_json::json!({
            "tool_name": "Edit",
            "tool_input": {
                "file_path": tmp.join("src/auth.ts").to_string_lossy().to_string(),
                "old_string": "old",
                "new_string": "new"
            }
        });

        let result = run_edit(&input.to_string());
        assert!(result.is_some());
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["decision"], "approve");
        assert!(json["reason"]
            .as_str()
            .unwrap()
            .contains("referenced in documentation"));
        assert!(json["additionalContext"]
            .as_str()
            .unwrap()
            .contains("arch.md"));
    }

    #[test]
    fn edit_returns_none_when_no_refs() {
        let tmp = setup_project(
            r#"{"chronicler":{}}"#,
            &[("arch.md", "No references here")],
            &["src/auth.ts"],
        );

        let input = serde_json::json!({
            "tool_name": "Edit",
            "tool_input": {
                "file_path": tmp.join("src/auth.ts").to_string_lossy().to_string(),
            }
        });

        assert!(run_edit(&input.to_string()).is_none());
    }

    #[test]
    fn edit_skips_md_files() {
        let tmp = setup_project(
            r#"{"chronicler":{}}"#,
            &[("arch.md", "See workspace/docs/arch.md:1")],
            &[],
        );

        let input = serde_json::json!({
            "tool_name": "Edit",
            "tool_input": {
                "file_path": tmp.join("workspace/docs/arch.md").to_string_lossy().to_string(),
            }
        });

        assert!(run_edit(&input.to_string()).is_none());
    }

    #[test]
    fn edit_respects_disabled_flag() {
        let tmp = setup_project(
            r#"{"chronicler":{"edit":false}}"#,
            &[("arch.md", "See src/auth.ts:42")],
            &["src/auth.ts"],
        );

        let input = serde_json::json!({
            "tool_name": "Edit",
            "tool_input": {
                "file_path": tmp.join("src/auth.ts").to_string_lossy().to_string(),
            }
        });

        assert!(run_edit(&input.to_string()).is_none());
    }

    #[test]
    fn stop_warn_mode() {
        let tmp = setup_project(
            r#"{"chronicler":{}}"#,
            &[("arch.md", "See src/auth.ts:42")],
            &["src/auth.ts"],
        );
        test_utils::set_mtime_past(&tmp.join("workspace/docs/arch.md"), 3600);

        let result = run_stop(&tmp);
        assert!(result.is_some());
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["decision"], "approve");
        assert!(json["reason"]
            .as_str()
            .unwrap()
            .contains("may be outdated"));
        assert!(json["additionalContext"]
            .as_str()
            .unwrap()
            .contains("arch.md"));
    }

    #[test]
    fn stop_block_mode() {
        let tmp = setup_project(
            r#"{"chronicler":{"mode":"block"}}"#,
            &[("arch.md", "See src/auth.ts:42")],
            &["src/auth.ts"],
        );
        test_utils::set_mtime_past(&tmp.join("workspace/docs/arch.md"), 3600);

        let result = run_stop(&tmp);
        assert!(result.is_some());
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["decision"], "block");
        assert!(json["reason"]
            .as_str()
            .unwrap()
            .contains("outdated"));
    }

    #[test]
    fn stop_fresh_docs_returns_none() {
        let tmp = setup_project(
            r#"{"chronicler":{}}"#,
            &[("arch.md", "See src/auth.ts:42")],
            &["src/auth.ts"],
        );
        test_utils::set_mtime_past(&tmp.join("src/auth.ts"), 3600);
        fs::write(tmp.join("workspace/docs/arch.md"), "See src/auth.ts:42").unwrap();

        assert!(run_stop(&tmp).is_none());
    }

    #[test]
    fn stop_respects_disabled_flag() {
        let tmp = setup_project(
            r#"{"chronicler":{"stop":false}}"#,
            &[("arch.md", "See src/auth.ts:42")],
            &["src/auth.ts"],
        );

        assert!(run_stop(&tmp).is_none());
    }

    #[test]
    fn stop_no_docs_dir_returns_none() {
        let tmp = TempDir::new("main-nodocs");
        fs::create_dir_all(tmp.join(".git")).unwrap();

        assert!(run_stop(&tmp).is_none());
    }

    #[test]
    fn edit_invalid_json_returns_none() {
        assert!(run_edit("not json").is_none());
    }

    #[test]
    fn edit_missing_file_path_returns_none() {
        let input = r#"{"tool_name":"Edit","tool_input":{}}"#;
        assert!(run_edit(input).is_none());
    }

    #[test]
    fn build_stop_output_warn() {
        let stale = vec![staleness::StaleDoc {
            doc_relative: "docs/arch.md".into(),
            stale_files: vec!["src/auth.ts".into()],
        }];
        let output = build_stop_output(&stale, config::Mode::Warn);
        assert_eq!(output["decision"], "approve");
        assert!(output["additionalContext"]
            .as_str()
            .unwrap()
            .contains("arch.md"));
    }

    #[test]
    fn build_stop_output_block() {
        let stale = vec![staleness::StaleDoc {
            doc_relative: "docs/arch.md".into(),
            stale_files: vec!["src/auth.ts".into()],
        }];
        let output = build_stop_output(&stale, config::Mode::Block);
        assert_eq!(output["decision"], "block");
        assert!(output["reason"]
            .as_str()
            .unwrap()
            .contains("1 document is outdated"));
    }

    #[test]
    fn build_stop_output_block_plural() {
        let stale = vec![
            staleness::StaleDoc {
                doc_relative: "docs/a.md".into(),
                stale_files: vec!["src/a.ts".into()],
            },
            staleness::StaleDoc {
                doc_relative: "docs/b.md".into(),
                stale_files: vec!["src/b.ts".into()],
            },
        ];
        let output = build_stop_output(&stale, config::Mode::Block);
        assert!(output["reason"]
            .as_str()
            .unwrap()
            .contains("2 documents are outdated"));
    }
}
