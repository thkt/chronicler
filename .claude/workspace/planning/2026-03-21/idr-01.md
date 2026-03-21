# IDR: PreToolUse gate hook + test-docs stateful tracking

> 2026-03-21

## Summary

stop hookの`mode: "block"`が無限ループを引き起こす構造的問題を解決するため、PreToolUse gate hookを新設。ドキュメントが古い状態でのソース編集をブロックし、更新後にリトライ可能。並行してtest-docs機能（stateful YAML lock）を追加し、テストファイルのドキュメンテーションライフサイクルを管理。3回の監査（/audit）と2回のpolish（/polish）を経てコード品質を確保。

## Changes

### [src/main.rs](file:////Users/thkt/GitHub/chronicler/src/main.rs)

```diff
@@ +23,6 @@ pub(crate) fn relative_path
+pub(crate) fn relative_path(path: &Path, root: &Path) -> String
+pub(crate) fn approve_with_context(reason: &str, context: &str) -> String
+pub(crate) fn canonicalize_within_root(path: &Path, root: &Path) -> Option<PathBuf>
+fn resolve_hook_docs(file_path_str: &str) -> Option<(...)>
+fn is_doc_stale_for_gate(doc_path: &Path, source_path: &Path) -> bool
+fn run_gate_for_path(file_path_str: &str) -> Option<String>
```

> [!NOTE]
>
> - `mode` フィールドと `Mode` enum を完全削除
> - `resolve_hook_docs` で edit/gate の共有プリアンブルを抽出（CQ-003/DUP-001解消）
> - `is_doc_stale_for_gate` でmtime比較を抽出（CQ-001: 58→35行）
> - `canonicalize_within_root` をedit/gate両パスに適用（SEC-003対応）
> - `format_edit_advisory` で `run_edit_for_path` を分割（CQ-007: 53→20行）
> - gate テスト8件追加（T-001〜T-008）

> [!TIP]
>
> - **PreToolUse gate**: stop hookのblockは無限ループ（持続的条件を解消不能）。PreToolUseなら条件解消可能
> - **Not adopted**: stop hook block修正 — 構造的に不可能（ADR-0003）
> - **exact path match only**: basename fallbackはadvisory(edit)専用。blockingでのfalse positiveはUX破壊的
> - **2秒tolerance**: formatter raceでmtimeが更新されるため

---

### [src/config.rs](file:////Users/thkt/GitHub/chronicler/src/config.rs)

```diff
@@ -1,45 +1,93 @@
-pub enum Mode { Warn, Block }
+pub struct ChroniclerConfig { gate: bool, ... }
+pub struct TestDocsConfig { enabled, patterns, output, layout, dir, language }
+pub fn load_both(project_dir) -> (ChroniclerConfig, TestDocsConfig)
```

> [!NOTE]
>
> - `Mode` enum 削除、`gate: bool` (default false) に置換
> - `TestDocsConfig` + `Layout` enum 追加（test-docs用）
> - `load_both()` で tools.json の二重読み込み解消
> - serde parse error に詳細追加（SF-001）

> [!TIP]
>
> - **gate opt-in**: blockingは破壊的UX変更。明示的有効化が必要
> - **load_both**: `ChroniclerConfig::load` + `TestDocsConfig::load` が同じファイルを2回パースしていた

---

### [src/td_hooks.rs](file:////Users/thkt/GitHub/chronicler/src/td_hooks.rs)

```diff
@@ +0,0 @@
+pub(crate) fn run_edit_test_docs_parsed(file_path_str: &str) -> Option<String>
+pub(crate) fn run_test_docs_check(project_dir: &Path) -> Option<String>
+pub(crate) fn run_test_docs_generate(project_dir: &Path) -> Option<String>
```

> [!NOTE]
>
> - main.rs から test-docs hook 関数群（~200行）を抽出（CQ-001: main.rs 548→354行）
> - `classify_test_files` でyaml不在時のファイル読み込みスキップ（EFF-004）
> - `find_orphaned_yamls` にパストラバーサル防止（SEC-004）
> - `build_edit_test_docs_prompt` 抽出（CQ-004: 49→37行）

---

### [src/test_discovery.rs](file:////Users/thkt/GitHub/chronicler/src/test_discovery.rs)

> [!NOTE]
>
> - glob パターンによるテストファイル探索
> - `symlink_metadata` でシングルstat（EFF-007）
> - `compile_file_patterns` で不正パターン警告（SF-003）

---

### [src/lock.rs](file:////Users/thkt/GitHub/chronicler/src/lock.rs)

> [!NOTE]
>
> - YAML lock ファイルの読み書き + `check_status` (Fresh/Stale/New/Orphaned)
> - `check_status` から `.exists()` TOCTOU 除去（EFF-002）

---

### [src/test_docs.rs](file:////Users/thkt/GitHub/chronicler/src/test_docs.rs)

> [!NOTE]
>
> - `Labels` struct で4x `match language` を1回に統合（CQ-002: 62→25行）

---

### [src/scanner.rs](file:////Users/thkt/GitHub/chronicler/src/scanner.rs)

> [!NOTE]
>
> - `walk_files_by_ext` 汎用化（walk_md_files → 拡張子パラメータ化）
> - `scan_docs` stat+open → single open + fd metadata（EFF-005）

---

### [CONTEXT.md](file:////Users/thkt/GitHub/chronicler/CONTEXT.md)

> [!NOTE]
>
> - Hook Event 0: PreToolUse (Gate) セクション追加
> - `gate` config フィールド記載
> - `mode` 関連記述削除

---

### [adr/0003-replace-stop-block-with-pretooluse-gate.md](file:////Users/thkt/GitHub/chronicler/adr/0003-replace-stop-block-with-pretooluse-gate.md)

> [!NOTE]
>
> - stop hook block → PreToolUse gate の設計判断を記録

---

### git diff --stat

```
 CONTEXT.md                                         |  69 +-
 Cargo.lock                                         | 139 ++++
 Cargo.toml                                         |   3 +
 adr/0002-add-stateful-test-docs-with-yaml-lock.md  |  73 ++
 adr/0003-replace-stop-block-with-pretooluse-gate.md|  77 +++
 adr/README.md                                      |   8 +-
 src/collector.rs                                   |   2 +-
 src/config.rs                                      | 244 +++++--
 src/hash.rs                                        |  29 +
 src/lock.rs                                        | 240 +++++++
 src/main.rs                                        | 754 ++++++++++++++-------
 src/scanner.rs                                     |  42 +-
 src/staleness.rs                                   |   8 +-
 src/td_hooks.rs                                    | 203 ++++++
 src/test_discovery.rs                              | 120 ++++
 src/test_docs.rs                                   | 150 ++++
 workspace/docs/architecture.md                     | 131 ++++
 workspace/docs/domain.md                           |  85 +++
 workspace/docs/setup.md                            |  94 +++
 19 files changed, 2132 insertions(+), 339 deletions(-)
```
