# Spec: テンプレートベースドキュメント生成

更新日: 2026-03-16
SOW: .claude/workspace/planning/2026-03-16-templates/sow.md

## 機能要件

| ID     | 説明                             | 入力                  | 出力                         | 実装対象 |
| ------ | -------------------------------- | --------------------- | ---------------------------- | -------- |
| FR-001 | テンプレート内蔵                 | -                     | 4 種類の &str 定数           | AC-1     |
| FR-002 | テンプレート書き出し             | project_root + config | ファイルシステムへの書き出し | AC-1     |
| FR-003 | テンプレートパス一覧取得         | project_root + config | Vec<PathBuf>                 | AC-3     |
| FR-004 | config に templates 追加         | .claude/tools.json    | ChroniclerConfig             | AC-2     |
| FR-005 | init プロンプトにパス含める      | SourceTree + パス一覧 | Markdown プロンプト          | AC-3     |
| FR-006 | update プロンプトにパス含める    | StaleDoc + パス一覧   | Markdown プロンプト          | AC-3     |
| FR-007 | check → 自動テンプレート書き出し | project_dir           | テンプレート + プロンプト    | AC-4     |
| FR-008 | init → 自動テンプレート書き出し  | project_dir           | テンプレート + プロンプト    | AC-4     |

バリデーション:

| FR     | ルール                       | エラー           |
| ------ | ---------------------------- | ---------------- |
| FR-002 | テンプレートディレクトリ既存 | 書き出しスキップ |
| FR-002 | 書き出し権限エラー           | eprintln + 続行  |
| FR-003 | テンプレートディレクトリ不在 | 空 Vec           |

## データモデル

```rust
// FR-001: template.rs
pub const TEMPLATE_NAMES: &[&str] = &["architecture", "api", "domain", "setup"];

// include_str! で内蔵
const DEFAULT_ARCHITECTURE: &str = include_str!("templates/architecture.md");
const DEFAULT_API: &str = include_str!("templates/api.md");
const DEFAULT_DOMAIN: &str = include_str!("templates/domain.md");
const DEFAULT_SETUP: &str = include_str!("templates/setup.md");

// FR-002
pub fn write_defaults(templates_dir: &Path) -> bool
// テンプレートディレクトリが存在しなければ作成 + 4 ファイル書き出し
// 存在すれば false を返す（スキップ）

// FR-003
pub fn list_template_paths(templates_dir: &Path) -> Vec<PathBuf>
// テンプレートディレクトリ内の .md ファイルパス一覧

// FR-004: config.rs 変更
pub struct ChroniclerConfig {
    pub dir: String,          // default: "workspace/docs"
    pub templates: String,    // default: "workspace/doc-templates"
    pub edit: bool,
    pub stop: bool,
    pub mode: Mode,
}
```

| モデル           | フィールド       | 使用元 |
| ---------------- | ---------------- | ------ |
| ChroniclerConfig | templates (新規) | FR-004 |
| TEMPLATE_NAMES   | (定数)           | FR-001 |

## 実装

| フェーズ | FRs                                    | ファイル                                  |
| -------- | -------------------------------------- | ----------------------------------------- |
| 1        | FR-001, FR-002, FR-003                 | src/template.rs, src/templates/\*.md      |
| 2        | FR-004, FR-005, FR-006, FR-007, FR-008 | src/config.rs, src/prompt.rs, src/main.rs |

## テストシナリオ

| ID    | タイプ | FR     | Given                        | When                           | Then                                   |
| ----- | ------ | ------ | ---------------------------- | ------------------------------ | -------------------------------------- |
| T-001 | unit   | FR-001 | -                            | テンプレート定数参照           | 4 種類すべて非空文字列                 |
| T-002 | unit   | FR-002 | テンプレートディレクトリなし | write_defaults() 呼び出し      | 4 ファイル書き出し、true 返却          |
| T-003 | unit   | FR-002 | テンプレートディレクトリあり | write_defaults() 呼び出し      | 書き出しスキップ、false 返却           |
| T-004 | unit   | FR-003 | テンプレート 4 ファイルあり  | list_template_paths() 呼び出し | 4 つの PathBuf 返却                    |
| T-005 | unit   | FR-004 | templates フィールドあり     | config load                    | config.templates が設定値              |
| T-006 | unit   | FR-004 | templates フィールドなし     | config load                    | config.templates がデフォルト値        |
| T-007 | unit   | FR-005 | SourceTree + パス 4 つ       | build_init_prompt()            | プロンプトにテンプレートパス含む       |
| T-008 | unit   | FR-006 | StaleDoc + パス 4 つ         | build_update_prompt()          | プロンプトにテンプレートパス含む       |
| T-009 | unit   | FR-007 | テンプレートなし + docs なし | run_check()                    | テンプレート書き出し + init プロンプト |
| T-010 | unit   | FR-008 | テンプレートなし             | run_init()                     | テンプレート書き出し + プロンプト出力  |

## 非機能要件

| ID      | カテゴリ   | 要件                           | 目標       | 検証対象 |
| ------- | ---------- | ------------------------------ | ---------- | -------- |
| NFR-001 | size       | バイナリサイズ増加             | <20KB      | AC-1     |
| NFR-002 | robustness | 書き出し失敗時の graceful skip | panic なし | AC-1     |

## 依存関係

| タイプ   | 名前         | 目的             | 使用元         |
| -------- | ------------ | ---------------- | -------------- |
| internal | collector.rs | ソースツリー収集 | FR-005         |
| internal | staleness.rs | stale 判定       | FR-006         |
| internal | sanitize.rs  | 出力制限         | FR-005, FR-006 |
| internal | config.rs    | 設定読み込み     | FR-004         |
| internal | prompt.rs    | プロンプト生成   | FR-005, FR-006 |

## 実装チェックリスト

- [ ] Phase 1: template.rs + 内蔵テンプレート (FR-001, FR-002, FR-003)
- [ ] Phase 2: config + prompt + main変更 (FR-004〜FR-008)

## トレーサビリティマトリクス

| AC   | FR                     | Test                | NFR              |
| ---- | ---------------------- | ------------------- | ---------------- |
| AC-1 | FR-001, FR-002         | T-001〜003          | NFR-001, NFR-002 |
| AC-2 | FR-004                 | T-005, T-006        | -                |
| AC-3 | FR-003, FR-005, FR-006 | T-004, T-007, T-008 | -                |
| AC-4 | FR-007, FR-008         | T-009, T-010        | -                |
