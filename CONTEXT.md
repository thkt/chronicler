# chronicler - Implementation Context

Claude Code hook tool for documentation lifecycle management (creation & update detection).

## What this tool does

1. PostToolUse (Write|Edit|MultiEdit): Detect when source file changes affect existing documentation → advisory notification
2. Stop: Check documentation freshness/coverage → warn or block

## Architecture Requirements

- Rust binary (edition 2024)
- Standalone (no shared library with other tools)
- Core hooks are stateless: reads fresh state on each invocation
- test-docs feature is stateful: uses per-file YAML for approval tracking (see ADR-0002)
- Git-aware: finds project root by walking `.git` boundary
- Graceful degradation: missing docs dir → skip, no error
- Minimal deps: serde, serde_json, regex

## Configuration

Reads from `.claude/tools.json` under `chronicler` key.

```json
{
  "chronicler": {
    "dir": "workspace/docs",
    "edit": true,
    "stop": true
  }
}
```

| Field  | Type   | Default          | Description                                          |
| ------ | ------ | ---------------- | ---------------------------------------------------- |
| `dir`  | string | `workspace/docs` | Directory to scan for documentation files (.md)      |
| `edit` | bool   | `true`           | Enable PostToolUse staleness notification            |
| `stop` | bool   | `true`           | Enable Stop freshness check                          |
| `gate` | bool   | `false`          | Enable PreToolUse gate (block edits when docs stale) |

## Hook Event 0: PreToolUse (Gate)

Trigger: `Write|Edit|MultiEdit` on source files (non-.md files)

### Logic

1. Parse stdin JSON, extract `file_path`
2. Skip if file is `.md`
3. If `gate` is false, exit silently
4. Scan docs for **exact path** references to the edited file (no basename fallback)
5. For each referencing doc, check staleness with **2-second tolerance** (handles formatter race)
6. If any stale docs found → block with update instructions
7. If all fresh or no references → silent pass

### Output (stdout JSON, only when stale docs found)

```json
{
  "decision": "block",
  "reason": "chronicler: documentation is stale. Update before editing `src/auth.ts`.\n\n- docs/architecture.md\n\nUpdate the listed documents, then retry your edit."
}
```

### Hook Registration

```json
{
  "matcher": "Write|Edit|MultiEdit",
  "hooks": [{ "type": "command", "command": "chronicler gate", "timeout": 3000 }]
}
```

## Hook Event 1: PostToolUse (Edit Detection)

Trigger: `Write|Edit|MultiEdit` on source files (non-.md files)

### Input (stdin JSON)

```json
{
  "tool_name": "Edit",
  "tool_input": {
    "file_path": "/absolute/path/to/file.ts",
    "old_string": "...",
    "new_string": "..."
  }
}
```

### Logic

1. Parse stdin JSON, extract `file_path`
2. Skip if file is `.md` (documentation itself being edited)
3. Find project root (walk ancestors for `.git`)
4. Load config from `.claude/tools.json` → `chronicler` section
5. If `edit` is false, exit silently
6. Scan `{project_root}/{dir}/**/*.md` for references to the edited file
7. Reference pattern: `file_path:line_number` (e.g., `src/utils/auth.ts:42`)
   - Match by relative path from project root
   - Also match basename if path is unique
8. If references found → stdout JSON with advisory

### Output (stdout JSON, only when references found)

```json
{
  "decision": "approve",
  "reason": "chronicler: edited file is referenced in documentation",
  "additionalContext": "## chronicler\n\nThe following docs may need updating:\n- docs/architecture.md (3 references to src/utils/auth.ts)\n- docs/api.md (1 reference to src/utils/auth.ts)\n\nRun `/docs architecture` to regenerate."
}
```

No output when no references found (silent pass).

## Hook Event 2: Stop (Freshness Gate)

Trigger: Session completion (Stop hook)

### Input

Command-line argument: `chronicler [project_dir]` (same pattern as `gates`)

### Logic

1. Receive project dir from args (default: `.`)
2. Find project root (walk ancestors for `.git`)
3. Load config
4. If `stop` is false, exit silently
5. Scan `{project_root}/{dir}/**/*.md` for all `file_path:line_number` references
6. For each referenced file:
   - Check if file still exists
   - Compare mtime of referenced file vs documentation file
   - If referenced file is newer → mark doc as stale
7. Collect stale docs

### Output

If stale docs found:

```json
{
  "decision": "approve",
  "reason": "chronicler: documentation may be outdated",
  "additionalContext": "## chronicler\n\nStale documentation detected:\n- docs/architecture.md (src/utils/auth.ts modified after doc generation)\n\nRun `/docs` to update."
}
```

No output when all docs are fresh.

## Reference Patterns to Scan

Documentation files contain references in these formats:

```
src/utils/auth.ts:42
`src/utils/auth.ts:42`
[src/utils/auth.ts:42]
```

Regex: `(?:^|[\s\[\`])([a-zA-Z0-9_./-]+\.[a-zA-Z0-9]+):(\d+)`

## Existing Hook Patterns to Follow

### Source Structure (follow `gates` pattern)

```
src/
  main.rs        # Entry point, dual-mode (stdin for PostToolUse, args for Stop)
  config.rs      # Load .claude/tools.json → chronicler section
  scanner.rs     # Scan docs dir for .md files, extract file references
  staleness.rs   # Compare mtimes, determine stale docs
  sanitize.rs    # ANSI strip, blank line compression (copy from gates)
  traverse.rs    # walk_ancestors for .git boundary (copy from gates)
  test_utils.rs  # TempDir helper (copy from gates)
```

### Dual-Mode Entry Point

Unlike other hooks that are single-event, chronicler handles two events:

```rust
fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Stop mode: chronicler [project_dir]
    if args.len() >= 2 || atty::is(atty::Stream::Stdin) {
        let dir = args.get(1).map(String::as_str).unwrap_or(".");
        run_stop(Path::new(dir));
        return;
    }

    // PostToolUse mode: reads stdin JSON
    run_edit();
}
```

Actually simpler: Stop hook passes no stdin, PostToolUse passes stdin JSON.
Detect by checking if stdin has data or by args.

### Config Pattern (from gates/config.rs)

```rust
const TOOLS_CONFIG_FILE: &str = ".claude/tools.json";

#[derive(Debug, PartialEq)]
pub struct ChroniclerConfig {
    pub dir: String,
    pub edit: bool,
    pub stop: bool,
    pub mode: String,  // "warn" | "block"
}

impl Default for ChroniclerConfig {
    fn default() -> Self {
        Self {
            dir: "workspace/docs".into(),
            edit: true,
            stop: true,
            mode: "warn".into(),
        }
    }
}
```

### Output Budget

- Per-doc detail: max 100 lines
- Total additionalContext: max 200KB (same as reviews)

### Cargo.toml

```toml
[package]
name = "chronicler"
version = "0.1.0"
edition = "2024"
description = "Claude Code hook for documentation lifecycle management"
license = "MIT"

[[bin]]
name = "chronicler"
path = "src/main.rs"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
regex = "1"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1
strip = true
```

## Hook Registration (settings.json)

PostToolUse (alongside formatter):

```json
{
  "matcher": "Write|Edit|MultiEdit",
  "hooks": [
    { "type": "command", "command": "formatter", "timeout": 2000 },
    { "type": "command", "command": "chronicler", "timeout": 3000 }
  ]
}
```

Stop (alongside gates):

```json
{
  "hooks": [{ "type": "command", "command": "chronicler", "timeout": 10000 }]
}
```

## Homebrew Formula

Add to `~/GitHub/homebrew-tap/Formula/chronicler.rb` (same pattern as gates.rb).

## Key Design Decisions

1. PostToolUse (edit) is advisory-only (never blocks). PreToolUse (gate) can block when docs are stale
2. Stop defaults to warn, not block — can be toggled via `mode: "block"`
3. Only scans `.md` files in configured `dir` — no recursive search outside
4. Skips when edited file is `.md` itself (avoid self-referential alerts)
5. Uses mtime comparison for staleness (simple, no git dependency for freshness)
6. No inter-tool communication: formatter and chronicler run independently on same PostToolUse event
