mod collector;
mod config;
mod hash;
mod lock;
mod prompt;
mod test_discovery;
mod td_hooks;
mod test_docs;
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

pub(crate) fn relative_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

pub(crate) fn approve_with_context(reason: &str, context: &str) -> String {
    serde_json::json!({
        "decision": "approve",
        "reason": reason,
        "additionalContext": context
    })
    .to_string()
}

pub(crate) fn canonicalize_within_root(path: &Path, root: &Path) -> Option<PathBuf> {
    let canonical = path.canonicalize().ok()?;
    let canonical_root = root.canonicalize().ok()?;
    canonical.starts_with(&canonical_root).then_some(canonical)
}

fn resolve_docs_dir(project_root: &Path, config_dir: &str) -> Option<PathBuf> {
    let docs_dir = project_root.join(config_dir);
    canonicalize_within_root(&docs_dir, project_root).or_else(|| {
        eprintln!(
            "chronicler: docs dir escapes project root: {}",
            config_dir
        );
        None
    })
}

pub(crate) fn parse_hook_file_path(input: &str) -> Option<String> {
    let json: serde_json::Value = match serde_json::from_str(input) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("chronicler: failed to parse hook input: {}", e);
            return None;
        }
    };
    json["tool_input"]["file_path"].as_str().map(String::from)
}

#[cfg(test)]
fn run_edit(input: &str) -> Option<String> {
    let file_path_str = parse_hook_file_path(input)?;
    run_edit_for_path(&file_path_str)
}

fn format_edit_advisory(
    matches: &[(&Path, usize)],
    target_relative: &str,
    project_root: &Path,
) -> String {
    let doc_lines: Vec<String> = matches
        .iter()
        .map(|(doc_path, count)| {
            let doc_rel = relative_path(doc_path, project_root);
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
    format!(
        "## Task\n\nCheck if the following documentation needs updating after editing `{}`.\n\n## Affected Documentation\n\n{}\n\n## Expected Output\n\nReview each affected document and update `file_path:line_number` references if needed.",
        target_relative, body
    )
}

fn resolve_hook_docs(
    file_path_str: &str,
) -> Option<(&Path, &Path, config::ChroniclerConfig, Vec<scanner::DocRefs>, String)> {
    if file_path_str.ends_with(".md") {
        return None;
    }
    let file_path = Path::new(file_path_str);
    let start = file_path.parent().unwrap_or(file_path);
    let project_root = traverse::find_project_root(start)?;
    if canonicalize_within_root(file_path, project_root).is_none() {
        return None;
    }
    let config = config::ChroniclerConfig::load(project_root);
    let docs_dir = resolve_docs_dir(project_root, &config.dir)?;
    let docs = scanner::scan_docs(&docs_dir);
    if docs.is_empty() {
        return None;
    }
    let target_relative = relative_path(file_path, project_root);
    Some((file_path, project_root, config, docs, target_relative))
}

fn run_edit_for_path(file_path_str: &str) -> Option<String> {
    let (_, project_root, config, docs, target_relative) = resolve_hook_docs(file_path_str)?;
    if !config.edit {
        return None;
    }

    let matches = scanner::find_refs_to_file(&docs, &target_relative);
    if matches.is_empty() {
        return None;
    }

    let context = format_edit_advisory(&matches, &target_relative, project_root);
    Some(approve_with_context(
        "chronicler: edited file is referenced in documentation",
        &context,
    ))
}

fn run_init(project_dir: &Path) -> Option<String> {
    let project_root = traverse::find_project_root(project_dir)?;
    let config = config::ChroniclerConfig::load(project_root);

    let templates_dir = project_root.join(&config.templates);
    template::write_defaults(&templates_dir);
    let template_paths = template::list_template_paths(&templates_dir);

    let tree = collector::collect_tree(project_root);
    let prompt_text = prompt::build_init_prompt(&tree, &config.dir, &template_paths);
    let context = sanitize::truncate_bytes(&prompt_text, MAX_CONTEXT_BYTES);

    Some(approve_with_context(
        "chronicler: initial documentation needed",
        &context,
    ))
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

    Some(approve_with_context(
        "chronicler: documentation update needed",
        &context,
    ))
}

fn run_check(project_dir: &Path) -> Option<String> {
    let project_root = traverse::find_project_root(project_dir)?;
    let (config, td_config) = config::load_both(project_root);
    if !config.stop {
        return None;
    }

    let templates_dir = project_root.join(&config.templates);
    template::write_defaults(&templates_dir);

    let docs_dir = match resolve_docs_dir(project_root, &config.dir) {
        Some(d) => d,
        None => return run_init(project_dir),
    };
    let docs = scanner::scan_docs(&docs_dir);

    if docs.is_empty() {
        return run_init(project_dir);
    }

    let stale = staleness::check_staleness(project_root, &docs);
    if stale.is_empty() {
        return td_hooks::run_test_docs_check_with_config(project_root, &td_config);
    }

    let template_paths = template::list_template_paths(&templates_dir);
    Some(build_check_output(&stale, &config.dir, &template_paths))
}

fn build_check_output(
    stale: &[staleness::StaleDoc],
    docs_dir: &str,
    template_paths: &[std::path::PathBuf],
) -> String {
    let prompt_text = prompt::build_update_prompt(stale, docs_dir, template_paths);
    let context = sanitize::truncate_bytes(&prompt_text, MAX_CONTEXT_BYTES);
    approve_with_context("chronicler: documentation may be outdated", &context)
}

#[cfg(test)]
fn run_edit_test_docs(input: &str) -> Option<String> {
    let file_path_str = parse_hook_file_path(input)?;
    td_hooks::run_edit_test_docs_parsed(&file_path_str)
}

fn run_edit_combined(input: &str) -> Option<String> {
    let file_path_str = parse_hook_file_path(input)?;

    run_edit_for_path(&file_path_str)
        .or_else(|| td_hooks::run_edit_test_docs_parsed(&file_path_str))
}

const GATE_TOLERANCE_SECS: u64 = 2;

fn run_gate(input: &str) -> Option<String> {
    let file_path_str = parse_hook_file_path(input)?;
    run_gate_for_path(&file_path_str)
}

fn is_doc_stale_for_gate(doc_path: &Path, source_path: &Path) -> bool {
    let doc_mtime = match doc_path.metadata().and_then(|m| m.modified()) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let source_mtime = match source_path.metadata().and_then(|m| m.modified()) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let tolerance = std::time::Duration::from_secs(GATE_TOLERANCE_SECS);
    source_mtime > doc_mtime + tolerance
}

fn run_gate_for_path(file_path_str: &str) -> Option<String> {
    let (file_path, project_root, config, docs, target_relative) =
        resolve_hook_docs(file_path_str)?;
    if !config.gate {
        return None;
    }

    // Gate uses exact path match only (no basename fallback)
    // to avoid false-positive blocking on unrelated files
    let stale_docs: Vec<String> = docs
        .iter()
        .filter(|doc| doc.file_refs.iter().any(|r| r == &target_relative))
        .filter(|doc| is_doc_stale_for_gate(&doc.doc_path, file_path))
        .map(|doc| relative_path(&doc.doc_path, project_root))
        .collect();

    if stale_docs.is_empty() {
        return None;
    }

    let doc_list = stale_docs
        .iter()
        .map(|d| format!("- {}", d))
        .collect::<Vec<_>>()
        .join("\n");
    let reason = format!(
        "chronicler: documentation is stale. Update before editing `{}`.\n\n{}\n\nUpdate the listed documents, then retry your edit.",
        target_relative, doc_list
    );
    Some(serde_json::json!({ "decision": "block", "reason": reason }).to_string())
}

fn shift_subcommand_args(args: &[String]) -> Vec<String> {
    [args[0].clone(), args[1].clone()]
        .into_iter()
        .chain(args.get(3..).unwrap_or_default().iter().cloned())
        .collect()
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
        Some("edit") => dispatch_stdin(run_edit_combined),
        Some("gate") => dispatch_stdin(run_gate),
        Some("init") => dispatch_dir(&args, run_init),
        Some("update") => dispatch_dir(&args, run_update),
        Some("check") => dispatch_dir(&args, run_check),
        Some("test-docs") => {
            let subcommand_args = shift_subcommand_args(&args);
            match args.get(2).map(String::as_str) {
                Some("check") => dispatch_dir(&subcommand_args, td_hooks::run_test_docs_check),
                Some("generate") => dispatch_dir(&subcommand_args, td_hooks::run_test_docs_generate),
                _ => {
                    eprintln!("chronicler: usage: chronicler test-docs <check|generate> [--project-dir PATH]");
                    std::process::exit(1);
                }
            }
        }
        Some(cmd) => {
            eprintln!("chronicler: unknown command: {}", cmd);
            std::process::exit(1);
        }
        None => {
            if std::io::stdin().is_terminal() {
                dispatch_dir(&args, run_check);
            } else {
                dispatch_stdin(run_edit_combined);
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
    fn check_ignores_legacy_block_mode() {
        let tmp = setup_project(
            r#"{"chronicler":{"mode":"block"}}"#,
            &[],
            &["src/main.rs"],
        );

        let result = run_check(&tmp);
        assert!(result.is_some());
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["decision"], "approve");
        assert!(json["reason"].as_str().unwrap().contains("initial documentation"));
    }

    #[test]
    fn build_check_output_contains_stale_doc() {
        let stale = vec![staleness::StaleDoc {
            doc_relative: "docs/arch.md".into(),
            stale_files: vec!["src/auth.ts".into()],
        }];
        let output = build_check_output(&stale, "workspace/docs", &[]);
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(json["decision"], "approve");
        assert!(json["additionalContext"].as_str().unwrap().contains("arch.md"));
    }

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

    fn count_md_files(dir: &Path) -> usize {
        fs::read_dir(dir)
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
            .count()
    }

    #[test]
    fn check_no_templates_writes_defaults_returns_init_prompt() {
        let tmp = setup_project(r#"{"chronicler":{}}"#, &[], &["src/main.rs"]);

        let templates_dir = tmp.join("workspace/doc-templates");
        assert!(!templates_dir.exists(), "templates dir should not exist before check");

        let result = run_check(&tmp);
        assert!(result.is_some(), "run_check should return Some");

        assert!(templates_dir.exists(), "templates dir should exist after check");
        assert_eq!(count_md_files(&templates_dir), 4);

        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["decision"], "approve");
        let ctx = json["additionalContext"].as_str().unwrap();
        assert!(ctx.contains("architecture.md"));
        assert!(ctx.contains("api.md"));
        assert!(ctx.contains("domain.md"));
        assert!(ctx.contains("setup.md"));
    }

    #[test]
    fn init_no_templates_writes_defaults_returns_prompt() {
        let tmp = setup_project(r#"{"chronicler":{}}"#, &[], &["src/main.rs"]);

        let templates_dir = tmp.join("workspace/doc-templates");
        assert!(!templates_dir.exists());

        let result = run_init(&tmp);
        assert!(result.is_some());

        assert!(templates_dir.exists());
        assert_eq!(count_md_files(&templates_dir), 4);

        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["decision"], "approve");
        assert!(json["reason"].as_str().unwrap().contains("initial documentation"));
        let ctx = json["additionalContext"].as_str().unwrap();
        assert!(ctx.contains("architecture.md"));
        assert!(ctx.contains("setup.md"));
    }

    fn setup_test_docs_project(
        config_json: &str,
        test_files: &[(&str, &str)],
    ) -> TempDir {
        let tmp = TempDir::new("test-docs");
        fs::create_dir_all(tmp.join(".git")).unwrap();
        fs::create_dir_all(tmp.join(".claude")).unwrap();
        fs::write(tmp.join(".claude/tools.json"), config_json).unwrap();

        for (path, content) in test_files {
            let file_path = tmp.join(path);
            if let Some(parent) = file_path.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            fs::write(file_path, content).unwrap();
        }

        tmp
    }

    // T-021: staleテストあり → additionalContext出力
    #[test]
    fn test_docs_check_stale_returns_context() {
        let tmp = setup_test_docs_project(
            r#"{"chronicler":{"testDocs":{"enabled":true,"patterns":["**/*.test.ts"],"dir":".test-docs"}}}"#,
            &[("src/auth.test.ts", "test('login', () => {})")],
        );

        let result = td_hooks::run_test_docs_check(&tmp);
        assert!(result.is_some());
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["decision"], "approve");
        assert!(json["reason"].as_str().unwrap().contains("test documentation"));
        let ctx = json["additionalContext"].as_str().unwrap();
        assert!(ctx.contains("auth.test.ts"));
        assert!(ctx.contains("new file"));
    }

    // T-022: staleテストなし → 出力なし(None)
    #[test]
    fn test_docs_check_fresh_returns_none() {
        let tmp = setup_test_docs_project(
            r#"{"chronicler":{"testDocs":{"enabled":true,"patterns":["**/*.test.ts"],"dir":".test-docs"}}}"#,
            &[("src/auth.test.ts", "test('login', () => {})")],
        );

        let test_content = fs::read(tmp.join("src/auth.test.ts")).unwrap();
        let current_hash = hash::content_hash(&test_content);
        let entry = lock::TestDocEntry {
            hash: current_hash,
            approved: Some("2026-03-20".into()),
            what: lock::L10n { en: "Auth tests".into(), ja: "認証テスト".into() },
            why: lock::L10n { en: "Auth matters".into(), ja: "認証は重要".into() },
            test_count: 1,
        };
        let yaml_path = tmp.join(".test-docs/src/auth.test.ts.yaml");
        lock::write_entry(&yaml_path, &entry).unwrap();

        let result = td_hooks::run_test_docs_check(&tmp);
        assert!(result.is_none());
    }

    // T-025: テストファイルをWrite/Edit → PostToolUse → stale通知
    #[test]
    fn edit_test_file_returns_test_docs_context() {
        let tmp = setup_test_docs_project(
            r#"{"chronicler":{"testDocs":{"enabled":true,"patterns":["**/*.test.ts"],"dir":".test-docs"}}}"#,
            &[("src/auth.test.ts", "test('login', () => {})")],
        );

        let input = serde_json::json!({
            "tool_name": "Edit",
            "tool_input": {
                "file_path": tmp.join("src/auth.test.ts").to_string_lossy().to_string(),
            }
        });

        let result = run_edit_test_docs(&input.to_string());
        assert!(result.is_some());
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["decision"], "approve");
        assert!(json["reason"].as_str().unwrap().contains("test documentation"));
        let ctx = json["additionalContext"].as_str().unwrap();
        assert!(ctx.contains("auth.test.ts"));
    }

    // T-026: 非テストファイルをWrite/Edit → PostToolUse → test-docs関連出力なし
    #[test]
    fn edit_non_test_file_returns_none_for_test_docs() {
        let tmp = setup_test_docs_project(
            r#"{"chronicler":{"testDocs":{"enabled":true,"patterns":["**/*.test.ts"],"dir":".test-docs"}}}"#,
            &[("src/app.ts", "const x = 1;")],
        );

        let input = serde_json::json!({
            "tool_name": "Edit",
            "tool_input": {
                "file_path": tmp.join("src/app.ts").to_string_lossy().to_string(),
            }
        });

        let result = run_edit_test_docs(&input.to_string());
        assert!(result.is_none());
    }

    // T-023: staleテストあり → stop hook(run_check)経由 → additionalContext出力
    #[test]
    fn check_integrates_test_docs_stale() {
        let tmp = setup_test_docs_project(
            r#"{"chronicler":{"testDocs":{"enabled":true,"patterns":["**/*.test.ts"],"dir":".test-docs"}}}"#,
            &[("src/auth.test.ts", "test('login', () => {})")],
        );
        fs::create_dir_all(tmp.join("workspace/docs")).unwrap();
        fs::write(tmp.join("workspace/docs/dummy.md"), "See placeholder.ts:1").unwrap();

        let result = run_check(&tmp);
        assert!(result.is_some());
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(json["additionalContext"].as_str().unwrap().contains("auth.test.ts"));
    }

    // T-024: staleテストなし → stop hook(run_check)経由 → 出力なし
    #[test]
    fn check_no_stale_test_docs_returns_none() {
        let tmp = setup_test_docs_project(
            r#"{"chronicler":{"testDocs":{"enabled":true,"patterns":["**/*.test.ts"],"dir":".test-docs"}}}"#,
            &[("src/auth.test.ts", "test('login', () => {})")],
        );
        let test_content = fs::read(tmp.join("src/auth.test.ts")).unwrap();
        let current_hash = hash::content_hash(&test_content);
        let entry = lock::TestDocEntry {
            hash: current_hash,
            approved: Some("2026-03-20".into()),
            what: lock::L10n { en: "Auth".into(), ja: "認証".into() },
            why: lock::L10n { en: "Matters".into(), ja: "重要".into() },
            test_count: 1,
        };
        lock::write_entry(&tmp.join(".test-docs/src/auth.test.ts.yaml"), &entry).unwrap();
        fs::create_dir_all(tmp.join("workspace/docs")).unwrap();
        fs::write(tmp.join("workspace/docs/dummy.md"), "See placeholder.ts:1").unwrap();

        let result = run_check(&tmp);
        assert!(result.is_none());
    }

    // SEC-002: symlinked file pointing outside project should be rejected
    #[test]
    fn edit_test_docs_rejects_symlink_outside_project() {
        let tmp = setup_test_docs_project(
            r#"{"chronicler":{"testDocs":{"enabled":true,"patterns":["**/*.test.ts"],"dir":".test-docs"}}}"#,
            &[("src/auth.test.ts", "test('login', () => {})")],
        );

        let outside = test_utils::TempDir::new("test-docs-outside");
        fs::write(outside.join("secret.test.ts"), "secret content").unwrap();
        std::os::unix::fs::symlink(
            outside.join("secret.test.ts"),
            tmp.join("src/evil.test.ts"),
        )
        .unwrap();

        let symlink_path = tmp.join("src/evil.test.ts");
        let result = td_hooks::run_edit_test_docs_parsed(&symlink_path.to_string_lossy());
        assert!(result.is_none(), "should reject symlinked file outside project root");
    }

    // === gate tests ===

    // T-001: gate:true, docs stale (>2s), file referenced → block
    #[test]
    fn gate_blocks_when_docs_stale() {
        let tmp = setup_project(
            r#"{"chronicler":{"gate":true}}"#,
            &[("arch.md", "See src/auth.ts:42")],
            &["src/auth.ts"],
        );
        test_utils::set_mtime_past(&tmp.join("workspace/docs/arch.md"), 3600);

        let result = run_gate_for_path(
            &tmp.join("src/auth.ts").to_string_lossy(),
        );
        assert!(result.is_some());
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(json["decision"], "block");
    }

    // T-002: gate:true, docs fresh → pass
    #[test]
    fn gate_passes_when_docs_fresh() {
        let tmp = setup_project(
            r#"{"chronicler":{"gate":true}}"#,
            &[("arch.md", "See src/auth.ts:42")],
            &["src/auth.ts"],
        );
        test_utils::set_mtime_past(&tmp.join("src/auth.ts"), 3600);
        fs::write(tmp.join("workspace/docs/arch.md"), "See src/auth.ts:42").unwrap();

        let result = run_gate_for_path(
            &tmp.join("src/auth.ts").to_string_lossy(),
        );
        assert!(result.is_none());
    }

    // T-003: gate:true, .md file → pass
    #[test]
    fn gate_skips_md_files() {
        let tmp = setup_project(
            r#"{"chronicler":{"gate":true}}"#,
            &[("arch.md", "See workspace/docs/arch.md:1")],
            &[],
        );

        let result = run_gate_for_path(
            &tmp.join("workspace/docs/arch.md").to_string_lossy(),
        );
        assert!(result.is_none());
    }

    // T-004: gate:false → pass
    #[test]
    fn gate_disabled_always_passes() {
        let tmp = setup_project(
            r#"{"chronicler":{}}"#,
            &[("arch.md", "See src/auth.ts:42")],
            &["src/auth.ts"],
        );
        test_utils::set_mtime_past(&tmp.join("workspace/docs/arch.md"), 3600);

        let result = run_gate_for_path(
            &tmp.join("src/auth.ts").to_string_lossy(),
        );
        assert!(result.is_none());
    }

    // T-005: gate:true, file not referenced → pass
    #[test]
    fn gate_passes_unreferenced_file() {
        let tmp = setup_project(
            r#"{"chronicler":{"gate":true}}"#,
            &[("arch.md", "See src/auth.ts:42")],
            &["src/auth.ts", "src/db.ts"],
        );
        test_utils::set_mtime_past(&tmp.join("workspace/docs/arch.md"), 3600);

        let result = run_gate_for_path(
            &tmp.join("src/db.ts").to_string_lossy(),
        );
        assert!(result.is_none());
    }

    // T-006: gate:true, no docs dir → pass
    #[test]
    fn gate_passes_no_docs_dir() {
        let tmp = setup_project(
            r#"{"chronicler":{"gate":true}}"#,
            &[],
            &["src/auth.ts"],
        );

        let result = run_gate_for_path(
            &tmp.join("src/auth.ts").to_string_lossy(),
        );
        assert!(result.is_none());
    }

    // T-007: gate + tolerance — source mtime within 2s of doc → pass
    #[test]
    fn gate_tolerance_passes_recent_changes() {
        let tmp = setup_project(
            r#"{"chronicler":{"gate":true}}"#,
            &[("arch.md", "See src/auth.ts:42")],
            &["src/auth.ts"],
        );
        let now = std::time::SystemTime::now();
        test_utils::set_mtime(&tmp.join("workspace/docs/arch.md"), now);
        let one_sec_later = now + std::time::Duration::from_secs(1);
        test_utils::set_mtime(&tmp.join("src/auth.ts"), one_sec_later);

        let result = run_gate_for_path(
            &tmp.join("src/auth.ts").to_string_lossy(),
        );
        assert!(result.is_none(), "should pass within 2s tolerance");
    }

    // T-008: gate + exact path — basename match only → pass (no block)
    #[test]
    fn gate_requires_exact_path_match() {
        let tmp = setup_project(
            r#"{"chronicler":{"gate":true}}"#,
            &[("arch.md", "See src/utils/auth.ts:42")],
            &["src/auth.ts"],
        );
        test_utils::set_mtime_past(&tmp.join("workspace/docs/arch.md"), 3600);

        let result = run_gate_for_path(
            &tmp.join("src/auth.ts").to_string_lossy(),
        );
        assert!(result.is_none(), "should not block on basename-only match");
    }
}
