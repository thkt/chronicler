# Spec: ドキュメントライフサイクル統合

更新日: 2026-03-16
SOW: .claude/workspace/planning/2026-03-16-doc-lifecycle/sow.md

## 機能要件

| ID     | 説明                          | 入力                   | 出力                         | 実装対象 |
| ------ | ----------------------------- | ---------------------- | ---------------------------- | -------- |
| FR-001 | ソース構造収集                | project_root パス      | ファイルツリー文字列         | AC-1     |
| FR-002 | init プロンプト生成           | ファイルツリー         | Markdown プロンプト          | AC-1     |
| FR-003 | update プロンプト生成         | StaleDoc リスト        | Markdown プロンプト          | AC-2     |
| FR-004 | init サブコマンド             | CLI args               | hook JSON (stdout)           | AC-1     |
| FR-005 | update サブコマンド           | CLI args               | hook JSON (stdout) or silent | AC-2     |
| FR-006 | check サブコマンド（旧 stop） | CLI args               | hook JSON (stdout) or silent | AC-3     |
| FR-007 | stop→check リネーム           | hooks 設定             | 正常動作                     | AC-4     |
| FR-008 | LLM 可読性改善                | 既存 additionalContext | 構造化プロンプト             | AC-5     |

バリデーション:

| FR     | ルール                                | エラー                |
| ------ | ------------------------------------- | --------------------- |
| FR-001 | project_root が存在しないディレクトリ | None を返す（silent） |
| FR-004 | project_dir が存在しないディレクトリ  | stderr + exit 1       |
| FR-005 | stale docs なし                       | None（出力なし）      |
| FR-006 | config.stop が false                  | None（出力なし）      |

## データモデル

```rust
// FR-001: collector.rs
pub struct SourceTree {
    pub entries: Vec<TreeEntry>,
}

pub struct TreeEntry {
    pub path: String,      // project_root からの相対パス
    pub is_dir: bool,
}

// FR-002, FR-003: prompt.rs
// 関数ベース。プロンプト文字列を返すだけで、構造体は不要。
// pub fn build_init_prompt(tree: &SourceTree, docs_dir: &str) -> String
// pub fn build_update_prompt(stale: &[StaleDoc], docs_dir: &str) -> String
```

| モデル     | フィールド   | 使用元 |
| ---------- | ------------ | ------ |
| SourceTree | entries      | FR-001 |
| TreeEntry  | path, is_dir | FR-001 |
| StaleDoc   | (既存)       | FR-003 |

## 実装

| フェーズ | FRs                    | ファイル                                          |
| -------- | ---------------------- | ------------------------------------------------- |
| 1        | FR-001, FR-002, FR-003 | src/collector.rs, src/prompt.rs                   |
| 2        | FR-004, FR-005, FR-006 | src/main.rs                                       |
| 3        | FR-007                 | src/config.rs, hooks/wrapper.sh, hooks/hooks.json |
| 4        | FR-008                 | src/main.rs, src/prompt.rs                        |

## テストシナリオ

| ID    | タイプ | FR     | Given                         | When                           | Then                                                 |
| ----- | ------ | ------ | ----------------------------- | ------------------------------ | ---------------------------------------------------- |
| T-001 | unit   | FR-001 | src/, lib/ を含むプロジェクト | collect_tree() 呼び出し        | 全ソースファイルが TreeEntry に含まれる              |
| T-002 | unit   | FR-001 | .git, node_modules が存在     | collect_tree() 呼び出し        | 除外ディレクトリがスキップされる                     |
| T-003 | unit   | FR-001 | ファイルが0個のプロジェクト   | collect_tree() 呼び出し        | 空の SourceTree が返る（panic しない）               |
| T-004 | unit   | FR-001 | symlink を含むディレクトリ    | collect_tree() 呼び出し        | symlink がスキップされる                             |
| T-005 | unit   | FR-002 | SourceTree（3ファイル）       | build_init_prompt() 呼び出し   | Markdown にファイルツリーと生成指示が含まれる        |
| T-006 | unit   | FR-002 | 空の SourceTree               | build_init_prompt() 呼び出し   | 最低限のプロンプトが返る（panic しない）             |
| T-007 | unit   | FR-003 | StaleDoc 2件                  | build_update_prompt() 呼び出し | 各 stale doc の情報と更新指示が含まれる              |
| T-008 | unit   | FR-004 | docs なしプロジェクト         | run_init(project_dir)          | hook JSON（init プロンプト付き）が返る               |
| T-009 | unit   | FR-005 | stale docs ありプロジェクト   | run_update(project_dir)        | hook JSON（update プロンプト付き）が返る             |
| T-010 | unit   | FR-005 | stale docs なしプロジェクト   | run_update(project_dir)        | None が返る                                          |
| T-011 | unit   | FR-006 | docs なしプロジェクト         | run_check(project_dir)         | hook JSON に init プロンプトが含まれる               |
| T-012 | unit   | FR-006 | stale docs ありプロジェクト   | run_check(project_dir)         | hook JSON に update プロンプトが含まれる             |
| T-013 | unit   | FR-006 | 全 docs 新鮮なプロジェクト    | run_check(project_dir)         | None が返る                                          |
| T-014 | unit   | FR-006 | config.stop = false           | run_check(project_dir)         | None が返る                                          |
| T-015 | unit   | FR-008 | edit で参照あり               | run_edit(input)                | additionalContext が「指示→コンテキスト→期待出力」順 |

## 非機能要件

| ID      | カテゴリ    | 要件                              | 目標               | 検証対象 |
| ------- | ----------- | --------------------------------- | ------------------ | -------- |
| NFR-001 | performance | init のツリー収集が高速           | <500ms（中規模PJ） | AC-1     |
| NFR-002 | size        | 出力サイズの制限                  | MAX_CONTEXT_LINES  | AC-1,2   |
| NFR-003 | robustness  | 読めないファイルでも panic しない | graceful skip      | AC-1     |

## 依存関係

| タイプ   | 名前         | 目的              | 使用元                 |
| -------- | ------------ | ----------------- | ---------------------- |
| internal | scanner.rs   | doc スキャン      | FR-005, FR-006         |
| internal | staleness.rs | stale 判定        | FR-003, FR-005, FR-006 |
| internal | sanitize.rs  | 出力制限          | FR-002, FR-003         |
| internal | traverse.rs  | project root 検出 | FR-004, FR-005, FR-006 |

## 実装チェックリスト

- [ ] Phase 1: collector.rs + prompt.rs新規作成 (FR-001, FR-002, FR-003)
- [ ] Phase 2: main.rsサブコマンド体系変更 (FR-004, FR-005, FR-006)
- [ ] Phase 3: config + hooks更新 (FR-007)
- [ ] Phase 4: LLM可読性改善 (FR-008)

## トレーサビリティマトリクス

| AC   | FR                     | Test                       | NFR                       |
| ---- | ---------------------- | -------------------------- | ------------------------- |
| AC-1 | FR-001, FR-002, FR-004 | T-001~006, T-008           | NFR-001, NFR-002, NFR-003 |
| AC-2 | FR-003, FR-005         | T-007, T-009, T-010        | NFR-002                   |
| AC-3 | FR-006                 | T-011, T-012, T-013, T-014 | -                         |
| AC-4 | FR-007                 | -                          | -                         |
| AC-5 | FR-008                 | T-015                      | -                         |
