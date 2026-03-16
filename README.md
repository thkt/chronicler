**English** | [日本語](README.ja.md)

# chronicler

Documentation lifecycle hook for Claude Code. Detects when source file edits affect existing documentation, checks freshness at session end, and provides template-based prompts for doc generation and updates.

## Features

| Feature             | Description                                                      |
| ------------------- | ---------------------------------------------------------------- |
| Edit detection      | PostToolUse hook alerts when edited files are referenced in docs |
| Freshness gate      | Stop hook warns or blocks when docs are stale (mtime-based)      |
| Template generation | 4 embedded templates build doc generation prompts                |
| Template overrides  | Users can customize templates; existing files are preserved      |
| Advisory-first      | PostToolUse never blocks; Stop defaults to warn mode             |
| Graceful            | Missing docs dir, bad config, unreadable files — all handled     |
| Configurable        | Per-project settings in `.claude/tools.json`                     |

## How It Works

```text
PostToolUse (Write/Edit/MultiEdit):
  Agent edits file → hook fires → chronicler edit
    ├─ Skips if edited file is .md
    ├─ Scans docs dir for file:line references to the edited file
    └─ If found → advisory JSON (approve + additionalContext)

Stop (session end):
  Agent completes → hook fires → chronicler check
    ├─ Writes default templates if missing
    ├─ Empty docs dir → init prompt (generation instructions)
    ├─ Compares mtime of referenced files vs doc files
    └─ If stale → warn (approve) or block based on mode setting
```

## Templates

chronicler embeds 4 documentation templates. On first run, they are written to the templates directory (default: `workspace/doc-templates/`).

| Template          | Purpose                                   |
| ----------------- | ----------------------------------------- |
| `architecture.md` | System overview (tech stack, structure)   |
| `api.md`          | API specification (endpoints, types)      |
| `domain.md`       | Domain model (glossary, entities)         |
| `setup.md`        | Developer onboarding (setup instructions) |

Each template includes section definitions, analysis techniques (Glob/Grep patterns), writing guidelines, and omit rules.

### Template Overrides

Place a file with the same name in the templates directory. chronicler will not overwrite it.

```text
workspace/doc-templates/
├── architecture.md   ← user-customized → preserved
├── api.md            ← missing → default written
├── domain.md         ← missing → default written
└── setup.md          ← missing → default written
```

Customized templates are automatically used in init/update/check prompts.

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

### Subcommands

| Command             | Description                                     |
| ------------------- | ----------------------------------------------- |
| `chronicler edit`   | Read edited file from stdin JSON, check refs    |
| `chronicler init`   | Write templates + initial doc generation prompt |
| `chronicler update` | Update prompt for stale documentation           |
| `chronicler check`  | Combined init + update (for Stop hook)          |

Without a subcommand: runs `check` if stdin is a terminal, `edit` if piped.

### Direct Execution

```bash
# edit mode (pipe JSON to stdin)
echo '{"tool_input":{"file_path":"/project/src/auth.ts"}}' | chronicler edit

# init (write templates + generation prompt)
chronicler init /path/to/project

# update (update prompt for stale docs)
chronicler update /path/to/project

# check (for Stop hook, combines init + update)
chronicler check /path/to/project
```

No output means no issues found.

## Configuration

Add a `chronicler` key to `.claude/tools.json` in your project root.

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

| Field       | Type   | Default                   | Description                                              |
| ----------- | ------ | ------------------------- | -------------------------------------------------------- |
| `dir`       | string | `workspace/docs`          | Directory to scan for documentation files (.md)          |
| `templates` | string | `workspace/doc-templates` | Templates directory (for override customization)         |
| `edit`      | bool   | `true`                    | Enable PostToolUse staleness notification                |
| `stop`      | bool   | `true`                    | Enable Stop freshness check                              |
| `mode`      | string | `"warn"`                  | Stop behavior: `"warn"` = advisory, `"block"` = blocking |

### Examples

Scan `docs/` directory, block on stale docs:

```json
{
  "chronicler": {
    "dir": "docs",
    "mode": "block"
  }
}
```

Custom templates directory:

```json
{
  "chronicler": {
    "templates": "my-templates"
  }
}
```

### Config Resolution

```text
project-root/
├── .claude/
│   └── tools.json     ← {"chronicler": {"dir": "docs", "templates": "my-templates"}}
├── .git/
├── workspace/
│   ├── docs/          ← generated documentation
│   └── doc-templates/ ← templates (customizable)
└── src/
```

## Output

### edit (advisory)

```json
{
  "decision": "approve",
  "reason": "chronicler: edited file is referenced in documentation",
  "additionalContext": "## Task\n\nCheck if the following documentation needs updating..."
}
```

### check — warn mode

```json
{
  "decision": "approve",
  "reason": "chronicler: documentation may be outdated",
  "additionalContext": "## Task\n\nUpdate the following stale documentation..."
}
```

### check — block mode

```json
{
  "decision": "block",
  "reason": "chronicler: 1 document is outdated.\n\n## docs/arch.md\nsrc/auth.ts modified after doc generation\n\nRun `chronicler update` to fix."
}
```

### init (no docs found)

```json
{
  "decision": "approve",
  "reason": "chronicler: initial documentation needed",
  "additionalContext": "## Task\n\nGenerate initial documentation for this project..."
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
