# SOW: ドキュメントライフサイクル統合

## Status

draft

## Overview

| Field      | Value                                                                             |
| ---------- | --------------------------------------------------------------------------------- |
| 目的       | chronicler にドキュメント生成・更新の指示機能を統合し、単一ツールで完結させる     |
| 対象       | src/main.rs, src/config.rs, src/prompt.rs (新規), src/collector.rs (新規), hooks/ |
| アプローチ | init/update サブコマンド追加 + stop→check リネーム + LLM 向けプロンプト出力       |
| 参照       | CONTEXT.md, README.md                                                             |

## Background

現在のchroniclerはドキュメントの「監視」のみを行い、生成・更新は外部の `/docs` スキルに依存している。これには以下の問題がある：

1. ドキュメントがゼロの状態ではchroniclerが何もしない（監視対象がない）
2. ユーザーが `/docs` の存在を知っている必要がある
3. プラグインインストール以外（Homebrew, リリースバイナリ）ではスキルが使えない
4. 「検知」と「対応」が別ツールに分かれており、認知負荷が高い

chroniclerを「ドキュメントライフサイクル全体を管理する単一ツール」に進化させる。生成自体はLLMに任せるが、chroniclerがプロンプトとコンテキストを組み立てて渡す「薄い連携」で実現する。

## Scope

### In Scope

| 対象             | 変更内容                                               | ファイル数 |
| ---------------- | ------------------------------------------------------ | ---------- |
| サブコマンド体系 | stop→check リネーム、init/update 追加                  | 2          |
| ソース構造収集   | プロジェクトのファイルツリー収集モジュール             | 1          |
| プロンプト組立   | LLM 向けプロンプト生成モジュール                       | 1          |
| check 拡張       | docs なし→init プロンプト、stale→update プロンプト出力 | 1          |
| LLM 可読性改善   | 既存 additionalContext の構造見直し                    | 1          |
| hooks 更新       | wrapper.sh, hooks.json の stop→check 変更              | 2          |
| 設定拡張         | init/update の有効化フラグ追加                         | 1          |

### Out of Scope

- chronicler自身がドキュメントを書く（LLMが書く）
- AST解析によるソース構造理解
- LLM APIの直接呼び出し
- 既存のeditモードの変更（PostToolUseの挙動は維持）

## Acceptance Criteria

### AC-1: init サブコマンド

- [ ] `chronicler init [project_dir]` でプロジェクトのソース構造を収集し、初期ドキュメント生成用プロンプトをJSONでstdoutに出力する
- [ ] 出力はhook JSON形式（`decision`, `reason`, `additionalContext`）に準拠する
- [ ] `additionalContext` 内のプロンプトがLLMにとって読みやすいMarkdown構造になっている

### AC-2: update サブコマンド

- [ ] `chronicler update [project_dir]` でstaleなドキュメントを特定し、更新用プロンプトをJSONでstdoutに出力する
- [ ] プロンプトには対象ドキュメント名、参照先ファイル、変更コンテキストが含まれる
- [ ] staleなドキュメントがない場合は出力なし（silent）

### AC-3: check サブコマンド（旧 stop）

- [ ] `chronicler check [project_dir]` が旧 `stop` と同等のfreshnessチェックを行う
- [ ] docsディレクトリが空またはなし → init相当のプロンプトを `additionalContext` に含めて出力
- [ ] stale docsあり → update相当のプロンプトを `additionalContext` に含めて出力
- [ ] docsが新鮮 → 出力なし

### AC-4: 後方互換

- [ ] hooks.jsonのstop→check変更が正しく動作する
- [ ] wrapper.shがcheckサブコマンドをまさしくルーティングする

### AC-5: LLM 可読性

- [ ] 既存のeditモードの `additionalContext` を見直し、LLMが指示として解釈しやすい構造にする
- [ ] プロンプト出力が「指示 → コンテキスト → 期待する出力」の順序になっている

## Implementation Plan

### Phase 1: ソース構造収集 + プロンプト組立

| ステップ | アクション                                        | ファイル数 |
| -------- | ------------------------------------------------- | ---------- |
| 1        | `src/collector.rs` 新規作成（ファイルツリー収集） | 1          |
| 2        | `src/prompt.rs` 新規作成（プロンプト組立）        | 1          |
| 3        | テスト追加                                        | 上記に含む |

Files: 2

### Phase 2: サブコマンド体系変更 + check 拡張

| ステップ | アクション                                                       | ファイル数 |
| -------- | ---------------------------------------------------------------- | ---------- |
| 1        | `src/main.rs` サブコマンドパーサー変更（init/update/check/edit） | 1          |
| 2        | `run_init`, `run_update` 関数追加                                | 上記に含む |
| 3        | `run_stop` → `run_check` リネーム + init/update 統合             | 上記に含む |
| 4        | テスト追加・既存テスト修正                                       | 上記に含む |

Files: 1

### Phase 3: 設定 + hooks 更新

| ステップ | アクション                                                | ファイル数 |
| -------- | --------------------------------------------------------- | ---------- |
| 1        | `src/config.rs` に init/update フラグ追加（必要に応じて） | 1          |
| 2        | `hooks/wrapper.sh` stop→check 変更                        | 1          |
| 3        | `hooks/hooks.json` stop→check 変更                        | 1          |

Files: 3

### Phase 4: LLM 可読性改善

| ステップ | アクション                                        | ファイル数 |
| -------- | ------------------------------------------------- | ---------- |
| 1        | 既存 edit モードの `additionalContext` 構造見直し | 1          |
| 2        | 全プロンプト出力の統一レビュー                    | 上記に含む |

Files: 1

## Test Plan

| テスト | 対象        | 検証内容                                             |
| ------ | ----------- | ---------------------------------------------------- |
| T-1    | collector   | ファイルツリー収集が正しい構造を返す                 |
| T-2    | collector   | .git, node_modules 等の除外ディレクトリをスキップ    |
| T-3    | collector   | 空プロジェクトでも panic しない                      |
| T-4    | prompt      | init プロンプトが期待する Markdown 構造を返す        |
| T-5    | prompt      | update プロンプトに stale doc 情報が含まれる         |
| T-6    | main/init   | docs なしプロジェクトでプロンプト JSON を出力        |
| T-7    | main/update | stale docs ありでプロンプト JSON を出力              |
| T-8    | main/update | stale docs なしで出力なし                            |
| T-9    | main/check  | docs なし → init プロンプト含む JSON 出力            |
| T-10   | main/check  | stale docs → update プロンプト含む JSON 出力         |
| T-11   | main/check  | docs 新鮮 → 出力なし                                 |
| T-12   | LLM可読性   | additionalContext が「指示→コンテキスト→期待出力」順 |

## Risks

| リスク                            | 影響 | 軽減策                                                        |
| --------------------------------- | ---- | ------------------------------------------------------------- |
| ソース構造収集の出力サイズ肥大    | MED  | MAX_CONTEXT_LINES 制限 + ファイルツリーのみ（内容は含めない） |
| 大規模プロジェクトでの収集速度    | LOW  | ツリー走査のみで高速。symlink スキップ済み                    |
| プロンプト品質が LLM の挙動に依存 | MED  | 構造化 Markdown で指示を明確化                                |
