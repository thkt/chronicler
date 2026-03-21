use std::path::PathBuf;

use crate::collector::SourceTree;
use crate::staleness::StaleDoc;

fn append_template_section(out: &mut String, template_paths: &[PathBuf]) {
    if template_paths.is_empty() {
        return;
    }
    out.push_str("## Templates\n\n");
    out.push_str("Read each template file below and follow its section structure when generating documentation.\n");
    out.push_str(
        "Each template contains section definitions, writing guidelines, and omit rules.\n\n",
    );
    for path in template_paths {
        out.push_str(&format!("- {}\n", path.display()));
    }
    out.push('\n');
}

pub fn build_init_prompt(tree: &SourceTree, docs_dir: &str, template_paths: &[PathBuf]) -> String {
    let mut out = String::new();

    out.push_str("## Task\n\n");
    out.push_str("Generate initial documentation for this project.\n");
    out.push_str(&format!(
        "Write documentation files to `{}/`.\n\n",
        docs_dir
    ));

    append_template_section(&mut out, template_paths);

    out.push_str("## Project Structure\n\n");
    if tree.entries.is_empty() {
        out.push_str("(no source files found)\n\n");
    } else {
        out.push_str("```\n");
        for entry in &tree.entries {
            if entry.is_dir {
                out.push_str(&format!("{}/\n", entry.path));
            } else {
                out.push_str(&format!("{}\n", entry.path));
            }
        }
        out.push_str("```\n\n");
    }

    out.push_str("## Expected Output\n\n");
    out.push_str("For each template, generate a corresponding documentation file:\n");
    out.push_str(&format!(
        "- architecture.md → `{}/architecture.md`\n",
        docs_dir
    ));
    out.push_str(&format!("- api.md → `{}/api.md`\n", docs_dir));
    out.push_str(&format!("- domain.md → `{}/domain.md`\n", docs_dir));
    out.push_str(&format!("- setup.md → `{}/setup.md`\n\n", docs_dir));
    out.push_str(
        "Skip a document entirely if the template's subject does not apply to this project ",
    );
    out.push_str("(e.g., skip api.md for a CLI tool with no API endpoints).\n");
    out.push_str("Use `file_path:line_number` references to link to source code.\n");

    out
}

pub fn build_update_prompt(
    stale: &[StaleDoc],
    docs_dir: &str,
    template_paths: &[PathBuf],
) -> String {
    let mut out = String::new();

    out.push_str("## Task\n\n");
    out.push_str("Update the following stale documentation.\n");
    out.push_str(&format!("Documentation directory: `{}/`\n\n", docs_dir));

    out.push_str("## Stale Documents\n\n");
    for s in stale {
        out.push_str(&format!("### {}\n\n", s.doc_relative));
        out.push_str("Modified source files:\n");
        for f in &s.stale_files {
            out.push_str(&format!("- {}\n", f));
        }
        out.push('\n');
    }

    append_template_section(&mut out, template_paths);

    out.push_str("## Expected Output\n\n");
    out.push_str("Update each stale document to reflect the current source code.\n");
    out.push_str("Verify every `file_path:line_number` reference by reading the source file.\n");
    out.push_str(
        "Update any references where the line number has drifted. Do not copy stale references.\n",
    );

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::TreeEntry;

    /// [T-005] when SourceTree has 3 files, should produce Markdown with file tree and instructions
    #[test]
    fn t_005_init_prompt_contains_file_tree_and_generation_instructions() {
        let tree = SourceTree {
            entries: vec![
                TreeEntry {
                    path: "src/main.rs".into(),
                    is_dir: false,
                },
                TreeEntry {
                    path: "src/lib.rs".into(),
                    is_dir: false,
                },
                TreeEntry {
                    path: "Cargo.toml".into(),
                    is_dir: false,
                },
            ],
        };

        let prompt = build_init_prompt(&tree, "workspace/docs", &[]);

        // File tree entries present
        assert!(
            prompt.contains("src/main.rs"),
            "expected src/main.rs in prompt"
        );
        assert!(
            prompt.contains("src/lib.rs"),
            "expected src/lib.rs in prompt"
        );
        assert!(
            prompt.contains("Cargo.toml"),
            "expected Cargo.toml in prompt"
        );

        // Docs directory referenced
        assert!(
            prompt.contains("workspace/docs"),
            "expected docs_dir in prompt"
        );

        // Generation instructions present
        assert!(
            prompt.contains("documentation"),
            "expected generation instructions in prompt"
        );
    }

    /// [T-006] when SourceTree is empty, should return minimal prompt without panic
    #[test]
    fn t_006_init_prompt_with_empty_tree_returns_minimal_prompt() {
        let tree = SourceTree { entries: vec![] };

        let prompt = build_init_prompt(&tree, "docs", &[]);

        assert!(!prompt.is_empty(), "prompt should not be empty");
        assert!(
            prompt.contains("docs"),
            "expected docs_dir in prompt even with empty tree"
        );
    }

    /// [T-007] when SourceTree has files and 4 template paths, init prompt contains all template paths
    #[test]
    fn t_007_init_prompt_includes_template_paths() {
        let tree = SourceTree {
            entries: vec![TreeEntry {
                path: "src/main.rs".into(),
                is_dir: false,
            }],
        };

        let template_paths: Vec<PathBuf> = vec![
            PathBuf::from("workspace/doc-templates/architecture.md"),
            PathBuf::from("workspace/doc-templates/api.md"),
            PathBuf::from("workspace/doc-templates/domain.md"),
            PathBuf::from("workspace/doc-templates/setup.md"),
        ];

        let prompt = build_init_prompt(&tree, "workspace/docs", &template_paths);

        assert!(
            prompt.contains("## Templates"),
            "expected Templates section in prompt"
        );
        for path in &template_paths {
            assert!(
                prompt.contains(&path.display().to_string()),
                "expected {} in prompt",
                path.display()
            );
        }
    }

    #[test]
    fn update_prompt_contains_stale_doc_info_and_instructions() {
        let stale = vec![
            StaleDoc {
                doc_relative: "workspace/docs/arch.md".into(),
                stale_files: vec!["src/auth.rs".into()],
            },
            StaleDoc {
                doc_relative: "workspace/docs/api.md".into(),
                stale_files: vec!["src/api.rs".into(), "src/routes.rs".into()],
            },
        ];

        let prompt = build_update_prompt(&stale, "workspace/docs", &[]);

        // Each stale doc info present
        assert!(prompt.contains("arch.md"), "expected arch.md in prompt");
        assert!(prompt.contains("api.md"), "expected api.md in prompt");

        // Stale source files present
        assert!(
            prompt.contains("src/auth.rs"),
            "expected src/auth.rs in prompt"
        );
        assert!(
            prompt.contains("src/api.rs"),
            "expected src/api.rs in prompt"
        );
        assert!(
            prompt.contains("src/routes.rs"),
            "expected src/routes.rs in prompt"
        );

        // Update instructions present
        assert!(
            prompt.contains("workspace/docs"),
            "expected docs_dir in prompt"
        );
    }

    /// [T-008] when StaleDoc entries and 4 template paths given, update prompt contains template paths
    #[test]
    fn t_008_update_prompt_includes_template_paths() {
        let stale = vec![StaleDoc {
            doc_relative: "workspace/docs/arch.md".into(),
            stale_files: vec!["src/auth.rs".into()],
        }];

        let template_paths: Vec<PathBuf> = vec![
            PathBuf::from("workspace/doc-templates/architecture.md"),
            PathBuf::from("workspace/doc-templates/api.md"),
            PathBuf::from("workspace/doc-templates/domain.md"),
            PathBuf::from("workspace/doc-templates/setup.md"),
        ];

        let prompt = build_update_prompt(&stale, "workspace/docs", &template_paths);

        assert!(
            prompt.contains("## Templates"),
            "expected Templates section in prompt"
        );
        for path in &template_paths {
            assert!(
                prompt.contains(&path.display().to_string()),
                "expected {} in prompt",
                path.display()
            );
        }
    }
}
