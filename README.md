**English** | [日本語](README.ja.md)

# chronicler

Documentation lifecycle hook for Claude Code. Detects when source file edits affect existing documentation and checks freshness at session end.

## Features

| Feature        | Description                                                      |
| -------------- | ---------------------------------------------------------------- |
| Edit detection | PostToolUse hook alerts when edited files are referenced in docs |
| Freshness gate | Stop hook warns or blocks when docs are stale (mtime-based)      |
| Advisory-first | PostToolUse never blocks; Stop defaults to warn mode             |
| Graceful       | Missing docs dir, bad config, unreadable files — all handled     |
| Configurable   | Per-project settings in `.claude/tools.json`                     |

## How It Works

```text
PostToolUse (Write/Edit/MultiEdit):
  Agent edits file → hook fires → chronicler reads stdin JSON
    ├─ Skips if edited file is .md
    ├─ Scans docs dir for file:line references to the edited file
    └─ If found → advisory JSON (approve + additionalContext)

Stop (session end):
  Agent completes → hook fires → chronicler checks freshness
    ├─ Scans docs dir for all file:line references
    ├─ Compares mtime of referenced files vs doc files
    └─ If stale → warn (approve) or block based on mode setting
```

## Reference Patterns

chronicler detects `file:line` references in documentation:

```
src/utils/auth.ts:42
`src/utils/auth.ts:42`
[src/utils/auth.ts:42]
```

## Installation

### Claude Code Plugin (recommended)

Installs the binary and registers both hooks automatically.

```bash
claude plugins marketplace add github:thkt/chronicler
claude plugins install chronicler
```

If the binary is not installed, run the bundled installer:

```bash
~/.claude/plugins/cache/chronicler/chronicler/*/hooks/install.sh
```

### Homebrew

```bash
brew install thkt/tap/chronicler
```

### From Release Binary

Download the latest binary from [Releases](https://github.com/thkt/chronicler/releases).

```bash
# macOS (Apple Silicon)
curl -L https://github.com/thkt/chronicler/releases/latest/download/chronicler-aarch64-apple-darwin.tar.gz | tar xz
mv chronicler ~/.local/bin/
```

### From Source

```bash
cd /tmp
git clone https://github.com/thkt/chronicler.git
cd chronicler
cargo build --release
cp target/release/chronicler ~/.local/bin/
cd .. && rm -rf chronicler
```

## Usage

### As a Claude Code Hook

Add to `~/.claude/settings.json`:

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Write|Edit|MultiEdit",
        "hooks": [{ "type": "command", "command": "chronicler", "timeout": 3000 }]
      }
    ],
    "Stop": [
      {
        "hooks": [{ "type": "command", "command": "chronicler", "timeout": 10000 }]
      }
    ]
  }
}
```

### Direct Execution

```bash
# PostToolUse mode (pipe JSON to stdin)
echo '{"tool_input":{"file_path":"/project/src/auth.ts"}}' | chronicler

# Stop mode
chronicler /path/to/project
```

No output means no issues found.

## Configuration

Add a `chronicler` key to `.claude/tools.json` in your project root.

```json
{
  "chronicler": {
    "dir": "workspace/docs",
    "edit": true,
    "stop": true,
    "mode": "warn"
  }
}
```

| Field  | Type   | Default          | Description                                              |
| ------ | ------ | ---------------- | -------------------------------------------------------- |
| `dir`  | string | `workspace/docs` | Directory to scan for documentation files (.md)          |
| `edit` | bool   | `true`           | Enable PostToolUse staleness notification                |
| `stop` | bool   | `true`           | Enable Stop freshness check                              |
| `mode` | string | `"warn"`         | Stop behavior: `"warn"` = advisory, `"block"` = blocking |

### Example

Scan `docs/` directory, block on stale docs:

```json
{
  "chronicler": {
    "dir": "docs",
    "mode": "block"
  }
}
```

## Output

### PostToolUse (advisory)

```json
{
  "decision": "approve",
  "reason": "chronicler: edited file is referenced in documentation",
  "additionalContext": "## chronicler\n\nThe following docs may need updating:\n- docs/arch.md (3 references to src/auth.ts)\n\nRun `/docs` to regenerate."
}
```

### Stop — warn mode

```json
{
  "decision": "approve",
  "reason": "chronicler: documentation may be outdated",
  "additionalContext": "## chronicler\n\nStale documentation detected:\n- docs/arch.md (src/auth.ts modified after doc generation)\n\nRun `/docs` to update."
}
```

### Stop — block mode

```json
{
  "decision": "block",
  "reason": "chronicler: 1 document is outdated.\n\n## docs/arch.md\nsrc/auth.ts modified after doc generation\n\nRun `/docs` to update."
}
```

## Companion Tools

This tool is part of a quality pipeline for Claude Code:

```bash
brew install thkt/tap/guardrails thkt/tap/formatter thkt/tap/reviews thkt/tap/gates thkt/tap/chronicler
```

| Tool                                             | Hook               | Timing            | Role                              |
| ------------------------------------------------ | ------------------ | ----------------- | --------------------------------- |
| [guardrails](https://github.com/thkt/guardrails) | PreToolUse         | Before Write/Edit | Lint + security checks            |
| [formatter](https://github.com/thkt/formatter)   | PostToolUse        | After Write/Edit  | Auto code formatting              |
| [reviews](https://github.com/thkt/reviews)       | PreToolUse         | Before Skill      | Static analysis context injection |
| [gates](https://github.com/thkt/gates)           | Stop               | Agent completion  | Quality gates (knip, tsgo, madge) |
| **chronicler**                                   | PostToolUse + Stop | Edit + completion | Documentation lifecycle           |

## License

MIT
