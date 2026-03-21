# テストドキュメント生成のためのstateful YAML管理を導入

- Status: accepted
- Deciders: thkt
- Date: 2026-03-20

## コンテキスト

chroniclerはCONTEXT.mdで「Stateless: reads fresh state on each invocation」を宣言しており、毎回まっさらな状態でmtime比較による陳腐化検知を行ってきた。

新機能「Living Test Docs」（`test-docs` サブコマンドとして実装。各テストファイルに対してLLMがWHAT（何を検証するか）とWHY（なぜ必要か）を推測し、人間が承認するハイブリッドワークフロー）を導入する。このワークフローには以下の永続状態が必要:

1. テストファイルのコンテンツハッシュ（変更検知用）
2. LLMが生成したWHAT/WHY（en/ja）
3. 人間の承認状態（approved日付）

これはchroniclerの「stateless」アイデンティティと矛盾する。

## 決定ドライバー

- 人間が承認した内容をLLMに上書きさせたくない（承認状態の永続化が必要）
- コンテンツハッシュで変更検知したい（mtimeはtouch/checkoutで誤検知する）
- 並行開発でのマージコンフリクトを最小化したい
- 既存のhook基盤・設定パターンを再利用したい

## 検討した選択肢

### A. テストファイルごとの個別YAMLファイル（採用）

テストファイル1つにつき1つのYAMLファイルを生成。配置は設定で選択可能:

- `centralized`: `.test-docs/src/rules/eval.rs.yaml`
- `collocated`: `src/rules/eval.rs.testdoc.yaml`

- Good: 並行開発でマージコンフリクトが発生しない（別ファイル＝別人＝衝突なし）
- Good: lock fileパターンは枯れた設計（Cargo.lock, package-lock.json等）
- Good: ファイル単位のhash比較で正確な変更検知
- Neutral: chroniclerがstatefulになる（アイデンティティの変更）
- Bad: ファイル数が増える（ただし専用ディレクトリに集約可能）

### B. 単一YAMLファイル（却下）

`.test-docs.lock.yaml` に全エントリを格納。

- Good: ファイル1つで管理がシンプル
- Bad: 並行開発でマージコンフリクトの温床になる（YAML mapはgitマージ戦略と相性が悪い）
- Bad: ファイルが大きくなる（300+テストファイル規模のプロジェクトでは管理困難）

### C. 新規ツールとして分離（却下）

`test-docs` として独立したRustバイナリを作成。

- Good: chroniclerのstatelessアイデンティティを維持
- Bad: hook基盤・設定パターンの再実装コスト
- Bad: ツール数の増加（メンテナンス負荷）
- Bad:「ドキュメントライフサイクル管理」の責務が分散

## 決定

選択肢Aを採用。chroniclerに `test-docs` サブコマンドを追加し、テストファイルごとの個別YAMLで承認状態を管理する。

### stateful化の範囲

- chroniclerの既存機能（mtime比較による陳腐化検知）はstatelessのまま変更しない
- test-docs機能のみがYAMLファイルを永続状態として使用する
- CONTEXT.mdを更新し、test-docs機能がstatefulであることを明記する

## 影響

- CONTEXT.mdの「Stateless」記述を更新した（Core hooks are stateless, test-docs feature is stateful）
- Cargo.tomlに `serde_yaml`, `sha2`, `glob` の依存を追加した
- `.test-docs/` ディレクトリまたは `*.testdoc.yaml` ファイルが新たにリポジトリに追加される
- Stop hook（`chronicler check`）でtest-docsの陳腐化検知が既存のmtime検知と独立して実行される。`testDocs.enabled: true` の場合のみ有効。既存の `stop` フラグとは別に管理する
