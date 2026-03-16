mod collector;
mod config;
mod prompt;
mod sanitize;
mod scanner;
mod staleness;
mod template;
#[cfg(test)]
mod test_utils;
mod traverse;

use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};

const MAX_CONTEXT_LINES: usize = 100;
const MAX_CONTEXT_BYTES: usize = 200_000; // 200KB Claude Code limit

fn resolve_docs_dir(project_root: &Path, config_dir: &str) -> Option<PathBuf> {
    let docs_dir = project_root.join(config_dir);
    let canonical = docs_dir.canonicalize().ok()?;
    let canonical_root = project_root.canonicalize().ok()?;
    if !canonical.starts_with(&canonical_root) {
        eprintln!(
            "chronicler: docs dir escapes project root: {}",
            config_dir
        );
        return None;
    }
    Some(canonical)
}

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

    let docs_dir = resolve_docs_dir(project_root, &config.dir)?;
    let docs = scanner::scan_docs(&docs_dir);
    if docs.is_empty() {
        return None;
    }

    let target_relative = file_path.strip_prefix(project_root).ok()?.to_string_lossy();

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
        "## Task\n\nCheck if the following documentation needs updating after editing `{}`.\n\n## Affected Documentation\n\n{}\n\n## Expected Output\n\nReview each affected document and update `file_path:line_number` references if needed.",
        target_relative, body
    );

    let output = serde_json::json!({
        "decision": "approve",
        "reason": "chronicler: edited file is referenced in documentation",
        "additionalContext": context
    });

    Some(output.to_string())
}

fn run_init(project_dir: &Path) -> Option<String> {
    run_init_with_mode(project_dir, config::Mode::Warn)
}

fn run_init_with_mode(project_dir: &Path, mode: config::Mode) -> Option<String> {
    let project_root = traverse::find_project_root(project_dir)?;
    let config = config::ChroniclerConfig::load(project_root);

    let templates_dir = project_root.join(&config.templates);
    template::write_defaults(&templates_dir);
    let template_paths = template::list_template_paths(&templates_dir);

    let tree = collector::collect_tree(project_root);
    let prompt_text = prompt::build_init_prompt(&tree, &config.dir, &template_paths);
    let context = sanitize::truncate_bytes(&prompt_text, MAX_CONTEXT_BYTES);

    let output = match mode {
        config::Mode::Block => {
            serde_json::json!({
                "decision": "block",
                "reason": format!("chronicler: no documentation found.\n\n{}", context)
            })
        }
        config::Mode::Warn => {
            serde_json::json!({
                "decision": "approve",
                "reason": "chronicler: initial documentation needed",
                "additionalContext": context
            })
        }
    };

    Some(output.to_string())
}

fn run_update(project_dir: &Path) -> Option<String> {
    let project_root = traverse::find_project_root(project_dir)?;
    let config = config::ChroniclerConfig::load(project_root);

    let docs_dir = resolve_docs_dir(project_root, &config.dir)?;
    let docs = scanner::scan_docs(&docs_dir);
    if docs.is_empty() {
        return None;
    }

    let stale = staleness::check_staleness(project_root, &docs);
    if stale.is_empty() {
        return None;
    }

    let templates_dir = project_root.join(&config.templates);
    let template_paths = template::list_template_paths(&templates_dir);

    let prompt_text = prompt::build_update_prompt(&stale, &config.dir, &template_paths);
    let context = sanitize::truncate_bytes(&prompt_text, MAX_CONTEXT_BYTES);

    let output = serde_json::json!({
        "decision": "approve",
        "reason": "chronicler: documentation update needed",
        "additionalContext": context
    });

    Some(output.to_string())
}

fn run_check(project_dir: &Path) -> Option<String> {
    let project_root = traverse::find_project_root(project_dir)?;
    let config = config::ChroniclerConfig::load(project_root);
    if !config.stop {
        return None;
    }

    let templates_dir = project_root.join(&config.templates);
    template::write_defaults(&templates_dir);

    let docs_dir = match resolve_docs_dir(project_root, &config.dir) {
        Some(d) => d,
        None => return run_init_with_mode(project_dir, config.mode),
    };
    let docs = scanner::scan_docs(&docs_dir);

    if docs.is_empty() {
        return run_init_with_mode(project_dir, config.mode);
    }

    let stale = staleness::check_staleness(project_root, &docs);
    if stale.is_empty() {
        return None;
    }

    let template_paths = template::list_template_paths(&templates_dir);

    Some(build_check_output(&stale, &config, &template_paths).to_string())
}

fn build_check_output(
    stale: &[staleness::StaleDoc],
    config: &config::ChroniclerConfig,
    template_paths: &[std::path::PathBuf],
) -> serde_json::Value {
    let prompt_text = prompt::build_update_prompt(stale, &config.dir, template_paths);

    match config.mode {
        config::Mode::Block => {
            let sections: Vec<String> = stale
                .iter()
                .map(|s| {
                    let files = s.stale_files.join(", ");
                    format!(
                        "## {}\n{} modified after doc generation",
                        s.doc_relative, files
                    )
                })
                .collect();
            let body = sanitize::tail_lines(&sections.join("\n\n"), MAX_CONTEXT_LINES);
            let reason = format!(
                "chronicler: {} {} outdated.\n\n{}\n\nRun `chronicler update` to fix.",
                stale.len(),
                if stale.len() == 1 {
                    "document is"
                } else {
                    "documents are"
                },
                body
            );
            serde_json::json!({ "decision": "block", "reason": reason })
        }
        config::Mode::Warn => {
            let context = sanitize::truncate_bytes(&prompt_text, MAX_CONTEXT_BYTES);
            serde_json::json!({
                "decision": "approve",
                "reason": "chronicler: documentation may be outdated",
                "additionalContext": context
            })
        }
    }
}

fn dispatch_dir(args: &[String], handler: fn(&Path) -> Option<String>) {
    let dir = args.get(2).map(String::as_str).unwrap_or(".");
    let project_dir = Path::new(dir);
    if !project_dir.is_dir() {
        eprintln!("chronicler: not a directory: {}", project_dir.display());
        return;
    }
    if let Some(json) = handler(project_dir) {
        println!("{}", json);
    }
}

fn dispatch_stdin(handler: fn(&str) -> Option<String>) {
    if std::io::stdin().is_terminal() {
        return;
    }
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("chronicler: failed to read stdin: {}", e);
        return;
    }
    if let Some(json) = handler(&input) {
        println!("{}", json);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("edit") => dispatch_stdin(run_edit),
        Some("init") => dispatch_dir(&args, run_init),
        Some("update") => dispatch_dir(&args, run_update),
        Some("check") => dispatch_dir(&args, run_check),
        Some(cmd) => {
            eprintln!("chronicler: unknown command: {}", cmd);
            std::process::exit(1);
        }
        None => {
            if std::io::stdin().is_terminal() {
                dispatch_dir(&args, run_check);
            } else {
                dispatch_stdin(run_edit);
            }
        }
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

        if !docs.is_empty() {
            let doc_dir = tmp.join("workspace/docs");
            fs::create_dir_all(&doc_dir).unwrap();
            for (name, content) in docs {
                let path = doc_dir.join(name);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).unwrap();
                }
                fs::write(path, content).unwrap();
            }
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

    // === edit tests (existing, updated) ===

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
        assert!(
            json["reason"]
                .as_str()
                .unwrap()
                .contains("referenced in documentation")
        );
        assert!(
            json["additionalContext"]
                .as_str()
                .unwrap()
                .contains("arch.md")
        );
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
    fn edit_invalid_json_returns_none() {
        assert!(run_edit("not json").is_none());
    }

    #[test]
    fn edit_missing_file_path_returns_none() {
        let input = r#"{"tool_name":"Edit","tool_input":{}}"#;
        assert!(run_edit(input).is_none());
    }

    // === update tests ===

    #[test]
    fn update_returns_prompt_when_stale() {
        let tmp = setup_project(
            r#"{"chronicler":{}}"#,
            &[("arch.md", "See src/auth.ts:42")],
            &["src/auth.ts"],
        );
        test_utils::set_mtime_past(&tmp.join("workspace/docs/arch.md"), 3600);

        let result = run_update(&tmp);
        assert!(result.is_some());
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["decision"], "approve");
        assert!(
            json["reason"]
                .as_str()
                .unwrap()
                .contains("documentation update needed")
        );
        let ctx = json["additionalContext"].as_str().unwrap();
        assert!(ctx.contains("arch.md"));
    }

    #[test]
    fn update_returns_none_when_fresh() {
        let tmp = setup_project(
            r#"{"chronicler":{}}"#,
            &[("arch.md", "See src/auth.ts:42")],
            &["src/auth.ts"],
        );
        test_utils::set_mtime_past(&tmp.join("src/auth.ts"), 3600);
        fs::write(tmp.join("workspace/docs/arch.md"), "See src/auth.ts:42").unwrap();

        assert!(run_update(&tmp).is_none());
    }

    // === check tests ===

    #[test]
    fn check_stale_docs_returns_update_prompt() {
        let tmp = setup_project(
            r#"{"chronicler":{}}"#,
            &[("arch.md", "See src/auth.ts:42")],
            &["src/auth.ts"],
        );
        test_utils::set_mtime_past(&tmp.join("workspace/docs/arch.md"), 3600);

        let result = run_check(&tmp);
        assert!(result.is_some());
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["decision"], "approve");
        assert!(json["reason"].as_str().unwrap().contains("may be outdated"));
    }

    #[test]
    fn check_fresh_docs_returns_none() {
        let tmp = setup_project(
            r#"{"chronicler":{}}"#,
            &[("arch.md", "See src/auth.ts:42")],
            &["src/auth.ts"],
        );
        test_utils::set_mtime_past(&tmp.join("src/auth.ts"), 3600);
        fs::write(tmp.join("workspace/docs/arch.md"), "See src/auth.ts:42").unwrap();

        assert!(run_check(&tmp).is_none());
    }

    #[test]
    fn check_respects_disabled_flag() {
        let tmp = setup_project(
            r#"{"chronicler":{"stop":false}}"#,
            &[("arch.md", "See src/auth.ts:42")],
            &["src/auth.ts"],
        );

        assert!(run_check(&tmp).is_none());
    }

    #[test]
    fn check_block_mode_no_docs_returns_block() {
        let tmp = setup_project(
            r#"{"chronicler":{"mode":"block"}}"#,
            &[],
            &["src/main.rs"],
        );

        let result = run_check(&tmp);
        assert!(result.is_some());
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["decision"], "block");
        assert!(json["reason"].as_str().unwrap().contains("no documentation found"));
    }

    // === build_check_output tests ===

    #[test]
    fn build_check_output_warn() {
        let stale = vec![staleness::StaleDoc {
            doc_relative: "docs/arch.md".into(),
            stale_files: vec!["src/auth.ts".into()],
        }];
        let config = config::ChroniclerConfig::default();
        let output = build_check_output(&stale, &config, &[]);
        assert_eq!(output["decision"], "approve");
        assert!(
            output["additionalContext"]
                .as_str()
                .unwrap()
                .contains("arch.md")
        );
    }

    #[test]
    fn build_check_output_block() {
        let stale = vec![staleness::StaleDoc {
            doc_relative: "docs/arch.md".into(),
            stale_files: vec!["src/auth.ts".into()],
        }];
        let config = config::ChroniclerConfig {
            mode: config::Mode::Block,
            ..config::ChroniclerConfig::default()
        };
        let output = build_check_output(&stale, &config, &[]);
        assert_eq!(output["decision"], "block");
        assert!(
            output["reason"]
                .as_str()
                .unwrap()
                .contains("1 document is outdated")
        );
    }

    // T-015: edit additionalContext follows instruction→context→expected output order
    #[test]
    fn edit_context_has_structured_prompt() {
        let tmp = setup_project(
            r#"{"chronicler":{}}"#,
            &[("arch.md", "See src/auth.ts:42 for auth logic")],
            &["src/auth.ts"],
        );

        let input = serde_json::json!({
            "tool_name": "Edit",
            "tool_input": {
                "file_path": tmp.join("src/auth.ts").to_string_lossy().to_string(),
            }
        });

        let result = run_edit(&input.to_string()).unwrap();
        let json: serde_json::Value = serde_json::from_str(&result).unwrap();
        let ctx = json["additionalContext"].as_str().unwrap();

        // Structure: Task (instruction) → Affected docs (context) → Expected Output
        let task_pos = ctx.find("## Task").expect("should have Task section");
        let affected_pos = ctx
            .find("## Affected Documentation")
            .expect("should have Affected Documentation section");
        let expected_pos = ctx
            .find("## Expected Output")
            .expect("should have Expected Output section");
        assert!(task_pos < affected_pos, "Task should come before Affected Documentation");
        assert!(affected_pos < expected_pos, "Affected Documentation should come before Expected Output");
    }

    // === template integration tests (T-009, T-010) ===

    /// [T-009] when project has no templates dir and no docs, run_check writes defaults then returns init prompt with template paths
    #[test]
    fn t_009_check_no_templates_writes_defaults_returns_init_prompt() {
        let tmp = setup_project(r#"{"chronicler":{}}"#, &[], &["src/main.rs"]);

        let templates_dir = tmp.join("workspace/doc-templates");
        assert!(!templates_dir.exists(), "templates dir should not exist before check");

        let result = run_check(&tmp);
        assert!(result.is_some(), "run_check should return Some");

        // Templates should have been written
        assert!(templates_dir.exists(), "templates dir should exist after check");
        let template_count = fs::read_dir(&templates_dir)
            .unwrap()
            .flatten()
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    == Some("md")
            })
            .count();
        assert_eq!(template_count, 4, "should have 4 template files");

        // Prompt should contain template paths
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["decision"], "approve");
        let ctx = json["additionalContext"].as_str().unwrap();
        assert!(
            ctx.contains("architecture.md"),
            "prompt should contain architecture template path"
        );
        assert!(
            ctx.contains("api.md"),
            "prompt should contain api template path"
        );
        assert!(
            ctx.contains("domain.md"),
            "prompt should contain domain template path"
        );
        assert!(
            ctx.contains("setup.md"),
            "prompt should contain setup template path"
        );
    }

    /// [T-010] when project has no templates dir, run_init writes defaults then returns prompt with template paths
    #[test]
    fn t_010_init_no_templates_writes_defaults_returns_prompt() {
        let tmp = setup_project(r#"{"chronicler":{}}"#, &[], &["src/main.rs"]);

        let templates_dir = tmp.join("workspace/doc-templates");
        assert!(!templates_dir.exists(), "templates dir should not exist before init");

        let result = run_init(&tmp);
        assert!(result.is_some(), "run_init should return Some");

        // Templates should have been written
        assert!(templates_dir.exists(), "templates dir should exist after init");
        let template_count = fs::read_dir(&templates_dir)
            .unwrap()
            .flatten()
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    == Some("md")
            })
            .count();
        assert_eq!(template_count, 4, "should have 4 template files");

        // Prompt should contain template paths
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["decision"], "approve");
        assert!(
            json["reason"]
                .as_str()
                .unwrap()
                .contains("initial documentation")
        );
        let ctx = json["additionalContext"].as_str().unwrap();
        assert!(
            ctx.contains("architecture.md"),
            "prompt should contain architecture template path"
        );
        assert!(
            ctx.contains("setup.md"),
            "prompt should contain setup template path"
        );
    }

    #[test]
    fn build_check_output_block_plural() {
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
        let config = config::ChroniclerConfig {
            mode: config::Mode::Block,
            ..config::ChroniclerConfig::default()
        };
        let output = build_check_output(&stale, &config, &[]);
        assert!(
            output["reason"]
                .as_str()
                .unwrap()
                .contains("2 documents are outdated")
        );
    }
}
