# stop hookのblock廃止とPreToolUse gate hookの新設

- Status: accepted
- Deciders: thkt
- Date: 2026-03-21

## コンテキスト

chroniclerのstop hookに `mode: "block"` 設定を追加し、ドキュメントが古い状態でのセッション終了をブロックする機能を実装した（`feat(check): support block mode for init prompt`）。

しかし、stop hook + `"decision": "block"` は構造的に無限ループを引き起こすことが判明した：

1. Claudeがターンを終了しようとする → stop hookが発火
2. ドキュメントが古い → `"decision": "block"` を返す
3. Claudeは応答を生成するが、ドキュメントの陳腐化は**持続的条件**であり、応答だけでは解消されない
4. Claudeが再びターンを終了 → stop hookが再発火 → 同じblock → 無限ループ

PreToolUse hookではこの問題が起きない。ブロックされた条件（ドキュメントが古い）をClaudeが**解消できる**（ドキュメントを更新してからリトライ）ため、ループは自然に終了する。

## 決定ドライバー

- ドキュメント更新を強制する仕組みが欲しい（advisoryだけでは無視される）
- stop hookのblockは構造的に無限ループ（解消不能な条件をblockできない）
- PreToolUseのblockは条件解消可能（Claudeがドキュメント更新→リトライできる）
- 既存のedit hook（PostToolUse advisory）との棲み分けが必要
- サブエージェント8体がchroniclerのstop hookにブロックされた実績あり（自己参照問題）

## 検討した選択肢

### A. stop hookのblock修正を試みる

stop hookで `"decision": "block"` を安全に使う方法を探す。

- Bad: 構造的に不可能。stop hookはターン終了時に発火し、blockはターン終了を阻止する。持続的条件では必ずループする
- Bad: Claude Codeのhook仕様としてstop + blockの組み合わせが想定されていない

### B. PreToolUse gate hookを新設（採用）

`mode: "block"` を廃止し、新しい `gate` サブコマンドをPreToolUse hookとして実装する。ソースファイル編集時に参照先ドキュメントの陳腐化をチェックし、古ければ編集をブロックする。

- Good: 条件解消可能（ドキュメント更新→リトライ）で無限ループしない
- Good: ファイル単位の粒度（stop hookはセッション全体）
- Good: opt-in（`gate: false` がデフォルト）で後方互換
- Neutral: edit hook（advisory）と並行して動作。edit = 編集後通知、gate = 編集前ブロック

### C. 外部ワークフローで強制

chronicler自体にはblocking機能を持たせず、CI/CDやpre-commitフックで強制する。

- Good: chroniclerの責務が小さく保てる
- Bad: リアルタイムのフィードバックがない（CI待ちが必要）
- Bad: Claude Codeのセッション内で完結しない

## 決定

**選択肢B: PreToolUse gate hookを新設。** `mode` フィールドを廃止し、`gate` フィールド（bool, default false）に置き換える。

## 設計詳細

### mtime tolerance (2秒)

PostToolUseのformatterがedit後にソースファイルのmtimeを更新する。gate hookがformatter完了前にmtimeを比較すると、formatterが更新した瞬間にfalse re-blockが発生する。2秒のtoleranceでこのrace conditionを吸収する。

### exact path matchのみ

edit hook（advisory）はbasename fallbackを使うが、gate hook（blocking）ではexact path matchのみ使用する。basename fallbackはfalse positiveのリスクがあり、blockingでのfalse positiveはUXを著しく損なうため。

### opt-in設計

gateはデフォルト無効（`gate: false`）。blockingは破壊的なUX変更であり、明示的に有効化を要求する。

## 影響

- `mode` フィールド（`"warn"` / `"block"`）を設定から削除。serde上は未知フィールドとして無視されるため後方互換
- stop hookは常にadvisory（`"decision": "approve"`）
- hook登録に `chronicler gate` をPreToolUse `Write|Edit|MultiEdit` matcherで追加
- テスト8件追加（T-001〜T-008: stale block, fresh pass, .md skip, disabled pass, no refs pass, no docs pass, tolerance pass, exact match only）
