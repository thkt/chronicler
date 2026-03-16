[English](README.md) | **日本語**

# chronicler

Claude Codeのドキュメントライフサイクルhook。ソースファイルの編集が既存ドキュメントに影響するかを検出し、セッション終了時に鮮度チェックを実行します。テンプレートベースのドキュメント生成・更新プロンプトも提供します。

## 特徴

| 機能               | 説明                                                                           |
| ------------------ | ------------------------------------------------------------------------------ |
| 編集検出           | PostToolUse hookが、編集されたファイルがドキュメントで参照されている場合に通知 |
| 鮮度ゲート         | Stop hookがドキュメントの鮮度を警告またはブロック（mtime比較）                 |
| テンプレート生成   | 4種の埋め込みテンプレートでドキュメント生成プロンプトを構築                    |
| テンプレート上書き | ユーザーがテンプレートをカスタマイズ可能（既存ファイルは保持）                 |
| アドバイザリ優先   | PostToolUseはブロックしない。Stopはデフォルトで警告モード                      |
| グレースフル       | docsディレクトリ未存在、設定不正、読み取り不可もすべてハンドリング             |
| 設定可能           | `.claude/tools.json` でプロジェクトごとに設定                                  |

## 仕組み

```text
PostToolUse (Write/Edit/MultiEdit):
  エージェントがファイル編集 → hook 発火 → chronicler edit
    ├─ 編集ファイルが .md ならスキップ
    ├─ docs ディレクトリで編集ファイルへの file:line 参照をスキャン
    └─ 参照あり → アドバイザリ JSON（approve + additionalContext）

Stop (セッション終了):
  エージェント完了 → hook 発火 → chronicler check
    ├─ テンプレートが未存在なら書き出し
    ├─ docs ディレクトリが空 → init プロンプト（生成指示）
    ├─ 参照先ファイルの mtime とドキュメントの mtime を比較
    └─ 古い場合 → モード設定に応じて warn（approve）または block
```

## テンプレート

chroniclerは4種のドキュメントテンプレートを内蔵しています。初回実行時にテンプレートディレクトリ（デフォルト: `workspace/doc-templates/`）へ自動的に書き出されます。

| テンプレート      | 用途                                       |
| ----------------- | ------------------------------------------ |
| `architecture.md` | システム概要（技術スタック、構成）         |
| `api.md`          | API仕様（エンドポイント、型）              |
| `domain.md`       | ドメインモデル（用語集、エンティティ）     |
| `setup.md`        | 開発者オンボーディング（セットアップ手順） |

各テンプレートにはセクション定義、分析テクニック（Glob/Grepパターン）、記述ガイドライン、省略ルールが含まれています。

### テンプレートの上書き

テンプレートディレクトリに同名のファイルを配置すると、chroniclerはそのファイルを上書きせず保持します。

```text
workspace/doc-templates/
├── architecture.md   ← ユーザーがカスタマイズ → そのまま保持
├── api.md            ← 未存在 → デフォルトを書き出し
├── domain.md         ← 未存在 → デフォルトを書き出し
└── setup.md          ← 未存在 → デフォルトを書き出し
```

カスタマイズしたテンプレートはinit/update/checkプロンプトに自動的に反映されます。

## 参照パターン

chroniclerはドキュメント内の `file:line` 参照を検出します。

```
src/utils/auth.ts:42
`src/utils/auth.ts:42`
[src/utils/auth.ts:42]
```

## インストール

### Claude Code Plugin（推奨）

バイナリのインストールとhookの登録が自動で行われます。

```bash
claude plugins marketplace add github:thkt/chronicler
claude plugins install chronicler
```

バイナリが未インストールの場合、同梱のインストーラを実行してください。

```bash
~/.claude/plugins/cache/chronicler/chronicler/*/hooks/install.sh
```

### Homebrew

```bash
brew install thkt/tap/chronicler
```

### リリースバイナリから

[Releases](https://github.com/thkt/chronicler/releases)から最新バイナリをダウンロードしてください。

```bash
# macOS (Apple Silicon)
curl -L https://github.com/thkt/chronicler/releases/latest/download/chronicler-aarch64-apple-darwin.tar.gz | tar xz
mv chronicler ~/.local/bin/
```

### ソースから

```bash
cd /tmp
git clone https://github.com/thkt/chronicler.git
cd chronicler
cargo build --release
cp target/release/chronicler ~/.local/bin/
cd .. && rm -rf chronicler
```

## 使い方

### Claude Code Hookとして

`~/.claude/settings.json` に追加してください。

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Write|Edit|MultiEdit",
        "hooks": [{ "type": "command", "command": "chronicler edit", "timeout": 3000 }]
      }
    ],
    "Stop": [
      {
        "hooks": [{ "type": "command", "command": "chronicler check", "timeout": 10000 }]
      }
    ]
  }
}
```

### サブコマンド

| コマンド            | 説明                                                  |
| ------------------- | ----------------------------------------------------- |
| `chronicler edit`   | stdin JSONから編集ファイルを読み取り、参照チェック    |
| `chronicler init`   | テンプレート書き出し + 初期ドキュメント生成プロンプト |
| `chronicler update` | 古いドキュメントの更新プロンプト                      |
| `chronicler check`  | init + update の統合（Stop hook用）                   |

サブコマンドなしの場合、stdinがターミナルなら `check`、パイプなら `edit` として動作します。

### 直接実行

```bash
# edit モード（stdin に JSON をパイプ）
echo '{"tool_input":{"file_path":"/project/src/auth.ts"}}' | chronicler edit

# init（テンプレート書き出し + 生成プロンプト）
chronicler init /path/to/project

# update（古いドキュメントの更新プロンプト）
chronicler update /path/to/project

# check（Stop hook用、init + update の統合）
chronicler check /path/to/project
```

出力がなければ問題なし。

## 設定

プロジェクトルートの `.claude/tools.json` に `chronicler` キーを追加します。

```json
{
  "chronicler": {
    "dir": "workspace/docs",
    "templates": "workspace/doc-templates",
    "edit": true,
    "stop": true,
    "mode": "warn"
  }
}
```

| フィールド  | 型     | デフォルト                | 説明                                                       |
| ----------- | ------ | ------------------------- | ---------------------------------------------------------- |
| `dir`       | string | `workspace/docs`          | スキャン対象のドキュメントディレクトリ（.md）              |
| `templates` | string | `workspace/doc-templates` | テンプレートディレクトリ（上書きカスタマイズ用）           |
| `edit`      | bool   | `true`                    | PostToolUse の鮮度通知を有効化                             |
| `stop`      | bool   | `true`                    | Stop の鮮度チェックを有効化                                |
| `mode`      | string | `"warn"`                  | Stop の動作: `"warn"` = アドバイザリ, `"block"` = ブロック |

### 設定例

`docs/` ディレクトリをスキャンし、古いドキュメントでブロックする構成です。

```json
{
  "chronicler": {
    "dir": "docs",
    "mode": "block"
  }
}
```

テンプレートディレクトリをカスタマイズする構成です。

```json
{
  "chronicler": {
    "templates": "my-templates"
  }
}
```

### 設定ファイルの解決

```text
project-root/
├── .claude/
│   └── tools.json     ← {"chronicler": {"dir": "docs", "templates": "my-templates"}}
├── .git/
├── workspace/
│   ├── docs/          ← 生成されたドキュメント
│   └── doc-templates/ ← テンプレート（カスタマイズ可能）
└── src/
```

## 出力

### edit（アドバイザリ）

```json
{
  "decision": "approve",
  "reason": "chronicler: edited file is referenced in documentation",
  "additionalContext": "## Task\n\nCheck if the following documentation needs updating..."
}
```

### check — warn モード

```json
{
  "decision": "approve",
  "reason": "chronicler: documentation may be outdated",
  "additionalContext": "## Task\n\nUpdate the following stale documentation..."
}
```

### check — block モード

```json
{
  "decision": "block",
  "reason": "chronicler: 1 document is outdated.\n\n## docs/arch.md\nsrc/auth.ts modified after doc generation\n\nRun `chronicler update` to fix."
}
```

### init（ドキュメント未存在時）

```json
{
  "decision": "approve",
  "reason": "chronicler: initial documentation needed",
  "additionalContext": "## Task\n\nGenerate initial documentation for this project..."
}
```

## 関連ツール

Claude Code向け品質パイプラインの一部です。

```bash
brew install thkt/tap/guardrails thkt/tap/formatter thkt/tap/reviews thkt/tap/gates thkt/tap/chronicler
```

| ツール                                           | Hook               | タイミング              | 役割                          |
| ------------------------------------------------ | ------------------ | ----------------------- | ----------------------------- |
| [guardrails](https://github.com/thkt/guardrails) | PreToolUse         | Write/Edit 前           | リント + セキュリティチェック |
| [formatter](https://github.com/thkt/formatter)   | PostToolUse        | Write/Edit 後           | 自動コード整形                |
| [reviews](https://github.com/thkt/reviews)       | PreToolUse         | レビュー系 Skill 実行時 | 静的解析コンテキスト提供      |
| [gates](https://github.com/thkt/gates)           | Stop               | エージェント完了時      | 品質ゲート (knip/tsgo/madge)  |
| **chronicler**                                   | PostToolUse + Stop | 編集時 + 完了時         | ドキュメントライフサイクル    |

## ライセンス

MIT
