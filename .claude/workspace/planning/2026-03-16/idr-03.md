# IDR: v0.2.0 バージョンバンプ

> 2026-03-16

## Summary

chronicler v0.1.1 → v0.2.0へのバージョンバンプ。ドキュメントライフサイクル統合（init/update/checkサブコマンド + テンプレートベース生成）の完了に伴うマイナーバージョン更新。ADR-0001をproposed → acceptedに変更。

## Changes

### [Cargo.toml](file:////Users/thkt/GitHub/chronicler/Cargo.toml)

- version: "0.1.1" → "0.2.0"

### [.claude-plugin/plugin.json](file:////Users/thkt/GitHub/chronicler/.claude-plugin/plugin.json)

- version: "0.1.1" → "0.2.0"
- description更新: テンプレートベース生成を反映

### [adr/0001-integrate-doc-lifecycle-into-chronicler.md](file:////Users/thkt/GitHub/chronicler/adr/0001-integrate-doc-lifecycle-into-chronicler.md)

- Status: proposed → accepted

### [adr/README.md](file:////Users/thkt/GitHub/chronicler/adr/README.md)

- ADR-0001のStatusをacceptedに更新

## Decisions

- semver minor bump（0.1.1 → 0.2.0）: サブコマンド体系の刷新 + テンプレート生成は後方互換性のない機能追加のため
