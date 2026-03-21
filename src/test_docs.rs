use crate::lock::TestDocEntry;
use std::collections::BTreeMap;

struct Labels {
    title: &'static str,
    file: &'static str,
    tests: &'static str,
    what: &'static str,
    why: &'static str,
    status: &'static str,
    approved: &'static str,
    draft: &'static str,
}

const LABELS_EN: Labels = Labels {
    title: "# Test Reference",
    file: "File",
    tests: "Tests",
    what: "What",
    why: "Why",
    status: "Status",
    approved: "Approved",
    draft: "Draft",
};

const LABELS_JA: Labels = Labels {
    title: "# テストリファレンス",
    file: "ファイル",
    tests: "テスト数",
    what: "何を検証するか",
    why: "なぜ必要か",
    status: "状態",
    approved: "承認済み",
    draft: "下書き",
};

fn l10n_text(l10n: &crate::lock::L10n, is_ja: bool) -> &str {
    if is_ja { &l10n.ja } else { &l10n.en }
}

pub fn generate(entries: &BTreeMap<String, TestDocEntry>, language: &str) -> String {
    let is_ja = language == "ja";
    let l = if is_ja { &LABELS_JA } else { &LABELS_EN };
    let mut lines = Vec::new();

    lines.push(l.title.to_string());
    lines.push(String::new());
    lines.push(format!(
        "| {} | {} | {} | {} | {} |",
        l.file, l.tests, l.what, l.why, l.status
    ));
    lines.push("|------|-------|------|-----|--------|".into());

    let mut total_tests: u32 = 0;
    for (file, entry) in entries {
        let status = if entry.approved.is_some() {
            l.approved
        } else {
            l.draft
        };
        lines.push(format!(
            "| {} | {} | {} | {} | {} |",
            file,
            entry.test_count,
            l10n_text(&entry.what, is_ja),
            l10n_text(&entry.why, is_ja),
            status
        ));
        total_tests += entry.test_count;
    }

    lines.push(String::new());
    if is_ja {
        lines.push(format!(
            "{} ファイル、{} テスト",
            entries.len(),
            total_tests
        ));
    } else {
        lines.push(format!("{} files, {} tests", entries.len(), total_tests));
    }
    lines.push(String::new());

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lock::L10n;

    fn sample_entries() -> BTreeMap<String, TestDocEntry> {
        let mut entries = BTreeMap::new();
        entries.insert(
            "src/rules/eval.rs".into(),
            TestDocEntry {
                hash: "sha256:abc".into(),
                approved: Some("2026-03-20".into()),
                what: L10n {
                    en: "Validates eval detection".into(),
                    ja: "eval検出を検証".into(),
                },
                why: L10n {
                    en: "Injection attacks".into(),
                    ja: "インジェクション攻撃".into(),
                },
                test_count: 8,
            },
        );
        entries.insert(
            "src/auth.test.ts".into(),
            TestDocEntry {
                hash: "sha256:def".into(),
                approved: None,
                what: L10n {
                    en: "Validates auth flow".into(),
                    ja: "認証フローを検証".into(),
                },
                why: L10n {
                    en: "Broken auth blocks users".into(),
                    ja: "認証破損はユーザーをブロック".into(),
                },
                test_count: 12,
            },
        );
        entries
    }

    // T-013: YAML群 + language="en" → 英語markdownが生成
    #[test]
    fn generates_english_markdown() {
        let entries = sample_entries();
        let md = generate(&entries, "en");

        assert!(md.starts_with("# Test Reference"));
        assert!(md.contains("| File |"));
        assert!(md.contains("Validates eval detection"));
        assert!(md.contains("Approved"));
        assert!(md.contains("Draft"));
        assert!(md.contains("2 files, 20 tests"));
    }

    // T-014: YAML群 + language="ja" → 日本語markdownが生成
    #[test]
    fn generates_japanese_markdown() {
        let entries = sample_entries();
        let md = generate(&entries, "ja");

        assert!(md.starts_with("# テストリファレンス"));
        assert!(md.contains("| ファイル |"));
        assert!(md.contains("eval検出を検証"));
        assert!(md.contains("承認済み"));
        assert!(md.contains("下書き"));
        assert!(md.contains("2 ファイル、20 テスト"));
    }

    // T-015: 同一YAML群 → generate 2回実行 → 同一markdown出力
    #[test]
    fn deterministic_output() {
        let entries = sample_entries();
        let md1 = generate(&entries, "en");
        let md2 = generate(&entries, "en");
        assert_eq!(md1, md2);
    }
}
