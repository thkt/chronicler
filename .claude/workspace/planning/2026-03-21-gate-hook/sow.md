# SOW: PreToolUse gate hook

## Status

draft

## Overview

| Field      | Value                                                            |
| ---------- | ---------------------------------------------------------------- |
| 目的       | stale ドキュメントが参照するファイルの編集を block する          |
| 対象       | src/main.rs, src/config.rs                                       |
| アプローチ | 既存の edit パイプライン（参照検出）+ staleness チェックの組合せ |
| 参照       | audit-2026-03-21-045035.yaml (mode 廃止の代替メカニズム)         |

## Background

chroniclerの `mode: "block"` はstop hookで使うと無限ループを引き起こすため廃止された。しかしwarnモードではClaudeがドキュメント更新通知を無視できる。PreToolUse hookで「staleなドキュメントが参照するファイルの編集」をblockすることで、無限ループなしにドキュメント更新を強制できる。

PreToolUse hookは個別のツール呼び出しをブロックするため、Claudeは別の作業（ドキュメント更新）をしてからリトライできる。条件が解消可能なので無限ループにならない。

## Scope

### In Scope

| 対象    | 変更内容                                        | ファイル数 |
| ------- | ----------------------------------------------- | ---------- |
| config  | `gate: bool` フィールド追加（default: false）   | 1          |
| main.rs | `run_gate`, `run_gate_for_path` 関数 + dispatch | 1          |
| テスト  | gate の block/pass/skip テスト                  | 同上       |
| CONTEXT | gate の説明追加                                 | 1          |

### Out of Scope

- test-docs (hash-based) のgateは対象外
- hook登録（settings.json）はユーザーが手動設定
- docsが存在しないプロジェクトでのgate動作（silent pass）

## Acceptance Criteria

### AC-1: stale ドキュメントが参照するファイルの編集が block される

- [ ] docs/arch.mdがsrc/auth.tsを参照、docsがstale → src/auth.tsのEditがblock
- [ ] block reasonにstaleドキュメント名と更新指示が含まれる

### AC-2: fresh なドキュメントが参照するファイルの編集は通過する

- [ ] docsがfresh → 編集はblockされない（出力なし）

### AC-3: .md ファイルの編集は常に通過する

- [ ] .mdファイルのEdit → gateは発火しない（docs更新をブロックしない）

### AC-4: gate は opt-in

- [ ] configに `gate` フィールドなし → gate無効（default: false）
- [ ] `gate: true` → gate有効
- [ ] `gate: false` → gate無効

### AC-5: ドキュメント参照がないファイルの編集は通過する

- [ ] docsがどこからも参照していないファイル → gateは発火しない

## Implementation Plan

### Phase 1: gate 実装

| ステップ | アクション                                      | ファイル数 |
| -------- | ----------------------------------------------- | ---------- |
| 1        | config.rs に `gate: bool` 追加 + load_both 更新 | 1          |
| 2        | main.rs に `run_gate`, `run_gate_for_path` 追加 | 1          |
| 3        | main.rs dispatch に `Some("gate")` 追加         | 0 (同上)   |
| 4        | テスト追加 (block/pass/skip/disabled)           | 0 (同上)   |
| 5        | CONTEXT.md に gate セクション追加               | 1          |

Files: 3

## Test Plan

| テスト | 対象              | 検証内容                                              |
| ------ | ----------------- | ----------------------------------------------------- |
| T-001  | gate + stale      | stale doc が参照するファイル → block                  |
| T-002  | gate + fresh      | fresh doc が参照するファイル → pass (None)            |
| T-003  | gate + .md        | .md ファイル編集 → pass (None)                        |
| T-004  | gate disabled     | gate: false → 常に pass (None)                        |
| T-005  | gate + no refs    | docs に参照がないファイル → pass (None)               |
| T-006  | gate + no docs    | docs ディレクトリなし → pass (None)                   |
| T-007  | gate + tolerance  | source mtime が doc mtime + 2秒以内 → pass (None)     |
| T-008  | gate + exact path | basename のみ一致する別ファイル → pass (block しない) |

## DA Challenge Findings (2026-03-21)

| #   | Finding                   | Resolution                                        |
| --- | ------------------------- | ------------------------------------------------- |
| 1   | mtime race with formatter | tolerance 2秒で回避 (T-007)                       |
| 2   | CONTEXT.md 矛盾           | gate セクション追加時に「never blocks」記述を更新 |
| 3   | basename false-positive   | gate は exact path match のみ使用 (T-008)         |

## Risks

| リスク                                | 影響 | 軽減策                            |
| ------------------------------------- | ---- | --------------------------------- |
| gate が意図せず有効になる             | HIGH | default: false (opt-in)           |
| formatter race で再ブロック           | MED  | 2秒 tolerance で回避 (DA #1)      |
| basename 誤検出で無関係ファイル block | MED  | exact path match のみ使用 (DA #3) |
