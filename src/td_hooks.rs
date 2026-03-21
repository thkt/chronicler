use std::path::{Path, PathBuf};

use crate::{
    approve_with_context, config, hash, lock, relative_path, scanner, test_discovery, test_docs,
    traverse,
};

pub(crate) fn run_edit_test_docs_parsed(file_path_str: &str) -> Option<String> {
    let file_path = Path::new(file_path_str);
    let start = file_path.parent().unwrap_or(file_path);
    let project_root = traverse::find_project_root(start)?;

    if crate::canonicalize_within_root(file_path, project_root).is_none() {
        eprintln!("chronicler: file escapes project root: {}", file_path_str);
        return None;
    }

    let td_config = config::TestDocsConfig::load(project_root);
    if !td_config.enabled {
        return None;
    }

    let compiled = test_discovery::compile_file_patterns(&td_config.patterns);
    if !test_discovery::is_test_file(file_path, &compiled) {
        return None;
    }

    let content = match std::fs::read(file_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("chronicler: cannot read {}: {}", file_path.display(), e);
            return None;
        }
    };
    let current_hash = hash::content_hash(&content);
    let yaml_path = td_config.yaml_path(project_root, file_path);
    let (status, _) = lock::check_status(&yaml_path, file_path, &current_hash);

    if matches!(status, lock::EntryStatus::Fresh) {
        return None;
    }

    let context = build_edit_test_docs_prompt(file_path, project_root, &yaml_path);
    Some(approve_with_context(
        "chronicler: test documentation needs updating",
        &context,
    ))
}

fn build_edit_test_docs_prompt(file_path: &Path, project_root: &Path, yaml_path: &Path) -> String {
    let relative = relative_path(file_path, project_root);
    format!(
        "## Task\n\nTest file `{}` was edited. Update its test documentation.\n\nRead the test file, infer WHAT it verifies and WHY it matters, then update the YAML:\n{}\n\nAfter updating, run: chronicler test-docs generate\n\n## Guidelines\n\n- WHAT: one sentence describing what the tests in this file verify\n- WHY: one sentence explaining why this matters\n- Provide both en and ja translations\n- Set approved to null\n- Set test_count by counting test functions/blocks",
        relative,
        yaml_path.display()
    )
}

fn find_orphaned_yamls(yaml_dir: &Path, project_root: &Path) -> Vec<String> {
    let mut orphaned = Vec::new();
    scanner::walk_files_by_ext(yaml_dir, "yaml", &mut |yaml_path| {
        let relative = yaml_path.strip_prefix(yaml_dir).unwrap_or(yaml_path);
        let rel_str = relative.to_string_lossy();
        let test_rel = rel_str.trim_end_matches(".yaml");
        if test_rel.contains("..") {
            return;
        }
        let test_path = project_root.join(test_rel);
        if !test_path.exists() {
            orphaned.push(test_rel.to_string());
        }
    });
    orphaned
}

fn classify_test_files(
    test_files: &[PathBuf],
    td_config: &config::TestDocsConfig,
    project_root: &Path,
) -> (Vec<String>, Vec<String>) {
    let mut stale = Vec::new();
    let mut new = Vec::new();
    for test_file in test_files {
        let yaml_path = td_config.yaml_path(project_root, test_file);
        let entry = lock::read_entry(&yaml_path);
        let relative = relative_path(test_file, project_root);

        if entry.hash.is_empty() {
            new.push(format!("- {} (new file, no YAML exists)", relative));
            continue;
        }

        let content = match std::fs::read(test_file) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("chronicler: cannot read {}: {}", test_file.display(), e);
                continue;
            }
        };
        let current_hash = hash::content_hash(&content);
        if entry.hash != current_hash {
            stale.push(format!(
                "- {} (hash changed: {}... → {}...)",
                relative,
                entry.hash.get(7..15).unwrap_or("?"),
                current_hash.get(7..15).unwrap_or("?")
            ));
        }
    }
    (stale, new)
}

fn build_test_docs_prompt(
    stale_files: Vec<String>,
    new_files: Vec<String>,
    orphaned_files: Vec<String>,
) -> String {
    let mut sections = Vec::new();
    sections.push("## Task\n\nThe following test files have changed since their documentation was last updated.\nRead each test file, infer WHAT it verifies and WHY it matters, then update\nthe corresponding .testdoc.yaml file.\n\nAfter updating the YAML files, run: chronicler test-docs generate".to_string());

    if !stale_files.is_empty() || !new_files.is_empty() {
        let mut items = Vec::new();
        items.extend(stale_files);
        items.extend(new_files);
        sections.push(format!(
            "## Files Requiring Updates\n\n{}",
            items.join("\n")
        ));
    }

    if !orphaned_files.is_empty() {
        let items: Vec<String> = orphaned_files
            .iter()
            .map(|f| format!("- {} (test file deleted)", f))
            .collect();
        sections.push(format!("## Orphaned Entries\n\n{}", items.join("\n")));
    }

    sections.push("## Guidelines\n\n- WHAT: one sentence describing what the tests in this file verify\n- WHY: one sentence explaining why this matters (the consequence of not testing)\n- Provide both en and ja translations\n- Set approved to null (human will review and approve)\n- Set test_count by counting test functions/blocks in the file".to_string());

    sections.join("\n\n")
}

pub(crate) fn run_test_docs_check(project_dir: &Path) -> Option<String> {
    let project_root = traverse::find_project_root(project_dir)?;
    let td_config = config::TestDocsConfig::load(project_root);
    run_test_docs_check_with_config(project_root, &td_config)
}

pub(crate) fn run_test_docs_check_with_config(
    project_root: &Path,
    td_config: &config::TestDocsConfig,
) -> Option<String> {
    if !td_config.enabled {
        return None;
    }

    let test_files = test_discovery::discover(project_root, &td_config.patterns);
    if test_files.is_empty() {
        return None;
    }

    let yaml_dir = project_root.join(&td_config.dir);
    let orphaned = find_orphaned_yamls(&yaml_dir, project_root);
    let (stale, new) = classify_test_files(&test_files, td_config, project_root);

    if stale.is_empty() && new.is_empty() && orphaned.is_empty() {
        return None;
    }

    let context = build_test_docs_prompt(stale, new, orphaned);
    Some(approve_with_context(
        "chronicler: test documentation needs updating",
        &context,
    ))
}

pub(crate) fn run_test_docs_generate(project_dir: &Path) -> Option<String> {
    let project_root = traverse::find_project_root(project_dir)?;
    let td_config = config::TestDocsConfig::load(project_root);

    let test_files = test_discovery::discover(project_root, &td_config.patterns);
    let mut entries = std::collections::BTreeMap::new();

    for test_file in &test_files {
        let yaml_path = td_config.yaml_path(project_root, test_file);
        if yaml_path.exists() {
            let relative = relative_path(test_file, project_root);
            let entry = lock::read_entry(&yaml_path);
            entries.insert(relative, entry);
        }
    }

    let markdown = test_docs::generate(&entries, &td_config.language);
    let output_path = project_root.join(&td_config.output);
    if let Some(parent) = output_path.parent()
        && let Err(e) = std::fs::create_dir_all(parent)
    {
        eprintln!("chronicler: cannot create dir {}: {}", parent.display(), e);
    }
    if let Err(e) = std::fs::write(&output_path, &markdown) {
        eprintln!("chronicler: cannot write {}: {}", output_path.display(), e);
    }

    None
}
