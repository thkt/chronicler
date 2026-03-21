# Spec: PreToolUse gate hook

更新日: 2026-03-21 SOW: .claude/workspace/planning/2026-03-21-gate-hook/sow.md

## 機能要件

| ID     | 説明                                 | 入力               | 出力           | 実装対象 |
| ------ | ------------------------------------ | ------------------ | -------------- | -------- |
| FR-001 | stale doc 参照ファイルの編集を block | stdin JSON         | block JSON     | AC-1     |
| FR-002 | fresh doc 参照ファイルの編集は通過   | stdin JSON         | None           | AC-2     |
| FR-003 | .md ファイルの編集は常に通過         | stdin JSON (.md)   | None           | AC-3     |
| FR-004 | gate は config で opt-in             | .claude/tools.json | gate 有効/無効 | AC-4     |
| FR-005 | 参照なしファイルの編集は通過         | stdin JSON         | None           | AC-5     |

バリデーション:

| FR     | ルール                                   | エラー              |
| ------ | ---------------------------------------- | ------------------- |
| FR-001 | stale = source mtime > doc mtime         | block + update 指示 |
| FR-003 | file_path が .md で終わる → skip         | N/A                 |
| FR-004 | config.gate が false または未設定 → skip | N/A                 |

## ドメインモデル

### データモデル

```rust
// config.rs (既存 ChroniclerConfig に追加)
pub struct ChroniclerConfig {
    pub dir: String,
    pub templates: String,
    pub edit: bool,
    pub stop: bool,
    pub gate: bool,  // FR-004: default false
}
```

| モデル           | フィールド | 使用元 |
| ---------------- | ---------- | ------ |
| ChroniclerConfig | gate       | FR-004 |

## 実装

| フェーズ | FRs            | ファイル                               |
| -------- | -------------- | -------------------------------------- |
| 1        | FR-001〜FR-005 | src/config.rs, src/main.rs, CONTEXT.md |

## テストシナリオ

| ID    | タイプ | FR     | Given                                          | When               | Then                              |
| ----- | ------ | ------ | ---------------------------------------------- | ------------------ | --------------------------------- |
| T-001 | unit   | FR-001 | gate:true, docs stale (>2s), file referenced   | gate に Edit stdin | decision:"block" + stale doc 一覧 |
| T-002 | unit   | FR-002 | gate:true, docs fresh, file referenced         | gate に Edit stdin | None                              |
| T-003 | unit   | FR-003 | gate:true, file_path が .md                    | gate に Edit stdin | None                              |
| T-004 | unit   | FR-004 | gate:false (or missing)                        | gate に Edit stdin | None                              |
| T-005 | unit   | FR-005 | gate:true, file not referenced by docs         | gate に Edit stdin | None                              |
| T-006 | unit   | FR-005 | gate:true, docs dir なし                       | gate に Edit stdin | None                              |
| T-007 | unit   | FR-001 | gate:true, source mtime within 2s of doc mtime | gate に Edit stdin | None (tolerance で pass)          |
| T-008 | unit   | FR-001 | gate:true, basename一致 but exact path不一致   | gate に Edit stdin | None (exact match のみ)           |

## 非機能要件

| ID      | カテゴリ    | 要件                          | 目標  | 検証対象 |
| ------- | ----------- | ----------------------------- | ----- | -------- |
| NFR-001 | performance | gate の実行時間               | <50ms | AC-1     |
| NFR-002 | safety      | .md 編集は絶対に block しない | 100%  | AC-3     |

## 依存関係

| タイプ   | 名前         | 目的                 | 使用元 |
| -------- | ------------ | -------------------- | ------ |
| internal | scanner.rs   | ドキュメント参照検出 | FR-001 |
| internal | staleness.rs | mtime 比較           | FR-001 |
| internal | config.rs    | gate フラグ読み込み  | FR-004 |

## 実装チェックリスト

- [ ] Phase 1: config.rsにgateフィールド追加 (FR-004)
- [ ] Phase 1: run_gate + run_gate_for_path実装 (FR-001〜FR-005)
- [ ] Phase 1: dispatchにgateコマンド追加
- [ ] Phase 1: テストT-001〜T-006追加
- [ ] Phase 1: CONTEXT.md更新

## トレーサビリティマトリクス

| AC   | FR     | Test         | NFR     |
| ---- | ------ | ------------ | ------- |
| AC-1 | FR-001 | T-001        | NFR-001 |
| AC-2 | FR-002 | T-002        |         |
| AC-3 | FR-003 | T-003        | NFR-002 |
| AC-4 | FR-004 | T-004        |         |
| AC-5 | FR-005 | T-005, T-006 |         |
