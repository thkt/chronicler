# IDR: check の block モード対応 + v0.2.2

> 2026-03-16

## Summary

`run_check` がdocsなし状態を検知した際、configの `mode` 設定に応じて `block` decisionを返せるようにした。blockモードではセッション終了がブロックされ、Claudeにドキュメント生成を強制する。v0.2.2バージョンバンプを含む。

## Changes

### [src/main.rs](file:////Users/thkt/GitHub/chronicler/src/main.rs)

- `run_init_with_mode(project_dir, mode)` を追加。modeがBlockなら `decision: "block"` で返す
- `run_init` は `run_init_with_mode(project_dir, Mode::Warn)` に委譲（手動呼び出し用、常にapprove）
- `run_check` が `run_init_with_mode(project_dir, config.mode)` を呼ぶように変更
- blockモードのテスト追加

### [.claude/tools.json](file:////Users/thkt/GitHub/chronicler/.claude/tools.json)

- chroniclerプロジェクト自体の設定: `mode: "block"` を有効化

### [workspace/doc-templates/](file:////Users/thkt/GitHub/chronicler/workspace/doc-templates/)

- `chronicler check` 初回実行時に自動書き出しされた4テンプレートファイル

### Version bump

- Cargo.toml, plugin.json: 0.2.1 → 0.2.2

## Decisions

- `run_init` は常にapprove（手動呼び出しでblockすると使いにくい）。blockは `run_check` 経由のみ
- chronicler自身のプロジェクトで `mode: "block"` を有効化してドッグフーディング
