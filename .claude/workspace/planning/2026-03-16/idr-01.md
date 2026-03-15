# IDR: chronicler 初期実装

> 2026-03-16

## Summary

Claude Code hookツールchroniclerの初期実装。PostToolUseでソースファイル編集時にドキュメント内の参照を検出し、Stopでドキュメントの鮮度をmtimeベースで検証する。Rust edition 2024、最小依存（serde, serde_json, regex）。Plugin + Homebrewのデュアル配布、CI/CDによる自動リリースパイプラインを含む。

## Changes

### [src/main.rs](file:////Users/thkt/GitHub/chronicler/src/main.rs)

```diff
@@ -0,0 +1,432 @@
+mod config;
+mod sanitize;
+mod scanner;
+mod staleness;
+mod traverse;
+
+fn run_edit(input: &str) -> Option<String> { ... }
+fn run_stop(project_dir: &Path) -> Option<String> { ... }
+fn build_stop_output(stale: &[StaleDoc], mode: Mode) -> Value { ... }
+fn main() { ... }
```

> [!NOTE]
>
> - デュアルモード entry point: stdin JSON → PostToolUse、CLI arg → Stop
> - `build_stop_output` を分離して warn/block の出力フォーマットを独立テスト可能に
> - `sanitize::tail_lines` で出力トランケーション（100行上限）
> - JSON parse / stdin read 失敗時に eprintln 診断出力

> [!TIP]
>
> - **stdin.is_terminal() でモード判定**: args + stdin 両方で判定。Plugin wrapper が mode 引数を渡すため安全
> - **Not adopted**: atty crate — `std::io::IsTerminal` が Rust 1.70+ で安定化済み

---

### [src/config.rs](file:////Users/thkt/GitHub/chronicler/src/config.rs)

```diff
@@ -0,0 +1,159 @@
+pub enum Mode { Warn, Block }
+pub struct ChroniclerConfig { dir, edit, stop, mode }
+impl ChroniclerConfig { pub fn load(project_dir) -> Self }
```

> [!NOTE]
>
> - `Mode` enum で型安全な mode 管理。unknown 値は eprintln + Warn フォールバック
> - `.claude/tools.json` の `chronicler` セクションから設定読み込み
> - 全フィールドにデフォルト値あり（config ファイル不要で動作）

> [!TIP]
>
> - **Mode::parse で手動パース**: unknown 値を eprintln + default に変換。serde enum だとパース失敗で config 全体がデフォルトに戻る
> - **Not adopted**: serde `#[serde(rename_all)]` on enum — パースエラー時に config 全体が失われるため

---

### [src/scanner.rs](file:////Users/thkt/GitHub/chronicler/src/scanner.rs)

```diff
@@ -0,0 +1,322 @@
+pub(crate) fn extract_refs(content: &str) -> Vec<String>
+pub fn scan_docs(docs_dir: &Path) -> Vec<DocRefs>
+pub fn find_refs_to_file(docs, target) -> Vec<(&Path, usize)>
+fn walk_md_files(dir, visitor)
```

> [!NOTE]
>
> - `extract_refs` を pub(crate) で公開し、regex ロジックの isolated テストを可能に
> - `walk_md_files`: `entry.file_type()` で syscall 削減、symlink スキップ
> - ファイルサイズ上限 1MiB + メタデータ読み取り失敗時の eprintln 診断
> - basename マッチ: 参照パス内で basename がユニークな場合のみフォールバック

> [!TIP]
>
> - **entry.file_type() で is_symlink + is_dir 判定**: DirEntry のメタデータを再利用し 3 syscalls → 1 に削減
> - **Not adopted**: walkdir crate — 15行の手書きウォーカーで十分。依存追加不要

---

### [src/staleness.rs](file:////Users/thkt/GitHub/chronicler/src/staleness.rs)

```diff
@@ -0,0 +1,141 @@
+pub fn check_staleness(project_root, docs) -> Vec<StaleDoc>
```

> [!NOTE]
>
> - doc 内参照ファイルの mtime と doc の mtime を比較。`ref_mtime > doc_mtime` で stale 判定
> - doc 内の参照は HashSet で重複排除してから stat
> - equal mtime は stale ではない（`>` not `>=`）— テストで明示的に検証

---

### [src/traverse.rs](file:////Users/thkt/GitHub/chronicler/src/traverse.rs)

```diff
@@ -0,0 +1,62 @@
+pub fn find_project_root(start: &Path) -> Option<&Path>
```

> [!NOTE]
>
> - `.git` ディレクトリを探して ancestor を walk（最大20階層）
> - gates と同じパターン

---

### [src/sanitize.rs](file:////Users/thkt/GitHub/chronicler/src/sanitize.rs)

```diff
@@ -0,0 +1,33 @@
+pub fn tail_lines(s: &str, max_lines: usize) -> String
```

> [!NOTE]
>
> - 出力トランケーション用。gates からコピーした `sanitize` 関数は audit で dead code として除去済み
> - `tail_lines` のみ production 使用

---

### [src/test_utils.rs](file:////Users/thkt/GitHub/chronicler/src/test_utils.rs)

```diff
@@ -0,0 +1,52 @@
+pub struct TempDir
+pub fn set_mtime_past(path, secs_ago)
+pub fn set_mtime(path, time)
```

> [!NOTE]
>
> - `set_mtime_past` / `set_mtime`: mtime 操作の DRY ヘルパー（6箇所で使用）

---

### [hooks/](file:////Users/thkt/GitHub/chronicler/hooks/)

> [!NOTE]
>
> - `hooks.json`: PostToolUse + Stop の両フック登録。wrapper に `edit`/`stop` mode を渡す
> - `wrapper.sh`: mode でルーティング。edit は stdin パイプ、stop は `chronicler .` 直接実行
> - `install.sh`: Homebrew → GitHub Releases フォールバック

---

### [.github/workflows/](file:////Users/thkt/GitHub/chronicler/.github/workflows/)

> [!NOTE]
>
> - `ci.yml`: push/PR で check + test + clippy + fmt
> - `release.yml`: tag push で 4プラットフォームビルド → GitHub Release → Homebrew formula 自動更新

---

### git diff --stat

```
 .claude-plugin/plugin.json    |  11 ++
 .github/workflows/ci.yml      |  42 ++++
 .github/workflows/release.yml | 170 +++++++++++++++++
 .gitignore                    |   3 +
 CONTEXT.md                    | 264 ++++++++++++++++++++++++++
 Cargo.lock                    | 146 ++++++++++++++
 Cargo.toml                    |  21 ++
 README.md                     | 207 ++++++++++++++++++++
 hooks/hooks.json              |  28 +++
 hooks/install.sh              |  80 ++++++++
 hooks/wrapper.sh              |  25 +++
 src/config.rs                 | 159 ++++++++++++++++
 src/main.rs                   | 432 ++++++++++++++++++++++++++++++++++++++++++
 src/sanitize.rs               |  33 ++++
 src/scanner.rs                | 322 +++++++++++++++++++++++++++++++
 src/staleness.rs              | 141 ++++++++++++++
 src/test_utils.rs             |  52 +++++
 src/traverse.rs               |  62 ++++++
 18 files changed, 2198 insertions(+)
```
