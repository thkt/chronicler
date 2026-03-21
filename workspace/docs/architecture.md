# chronicler - Architecture Overview

## Technology Stack

| Category | Technology    | Version      |
| -------- | ------------- | ------------ |
| Language | Rust          | 2024 edition |
| Runtime  | Native binary | -            |

## Directory Structure

```
src/
  collector.rs
  config.rs
  main.rs
  prompt.rs
  sanitize.rs
  scanner.rs
  staleness.rs
  template.rs
  templates/
    api.md
    architecture.md
    domain.md
    setup.md
  test_utils.rs
  traverse.rs
hooks/
  hooks.json
  install.sh
  wrapper.sh
adr/
  0001-integrate-doc-lifecycle-into-chronicler.md
  README.md
```

## Module Structure

```mermaid
graph TD
    main["src/main.rs"] --> config["src/config.rs"]
    main --> collector["src/collector.rs"]
    main --> prompt["src/prompt.rs"]
    main --> scanner["src/scanner.rs"]
    main --> staleness["src/staleness.rs"]
    main --> template["src/template.rs"]
    main --> sanitize["src/sanitize.rs"]
    main --> traverse["src/traverse.rs"]
    prompt --> collector
    prompt --> staleness
    staleness --> scanner
```

## Data Flow

### PostToolUse (edit mode)

```mermaid
sequenceDiagram
    participant CC as Claude Code
    participant W as wrapper.sh
    participant M as main.rs
    participant S as scanner.rs

    CC->>W: PostToolUse hook (stdin JSON)
    W->>M: chronicler edit (pipe stdin)
    M->>M: parse file_path from JSON
    M->>M: find_project_root()
    M->>S: scan_docs() + find_refs_to_file()
    S-->>M: matching docs
    M-->>CC: approve JSON with additionalContext
```

### Stop (check mode)

```mermaid
sequenceDiagram
    participant CC as Claude Code
    participant W as wrapper.sh
    participant M as main.rs
    participant T as template.rs

    CC->>W: Stop hook
    W->>M: chronicler check .
    M->>T: write_defaults() (if needed)
    M->>M: scan_docs()
    alt no docs
        M-->>CC: block/approve with init prompt
    else stale docs
        M-->>CC: block/approve with update prompt
    else fresh
        M-->>CC: (no output)
    end
```

## Key Components

| Component   | Path                  | Description                                                              |
| ----------- | --------------------- | ------------------------------------------------------------------------ |
| Entry point | `src/main.rs:1`       | Dual-mode CLI: dispatches edit/init/update/check subcommands             |
| Config      | `src/config.rs:27`    | Loads `ChroniclerConfig` from `.claude/tools.json` with defaults         |
| Scanner     | `src/scanner.rs:23`   | Scans docs for `file:line` references via regex                          |
| Staleness   | `src/staleness.rs:10` | Compares mtime of referenced files vs doc files                          |
| Collector   | `src/collector.rs:24` | Walks project tree, skipping .git/node_modules/target                    |
| Prompt      | `src/prompt.rs:19`    | Builds init/update prompts with template paths and file tree             |
| Template    | `src/template.rs:21`  | Embeds default templates via `include_str!`, writes missing ones to disk |
| Sanitize    | `src/sanitize.rs:1`   | Output truncation: `truncate_bytes` (200KB) and `tail_lines` (100 lines) |
| Traverse    | `src/traverse.rs:5`   | Finds project root by walking ancestors for `.git`                       |

## Dependencies

### External

| Crate      | Purpose                                         |
| ---------- | ----------------------------------------------- |
| serde      | JSON deserialization for config and hook input  |
| serde_json | JSON serialization/deserialization for hook I/O |
| regex      | `file:line` reference pattern matching in docs  |

### Internal

- `main` → `config`: loads project configuration
- `main` → `scanner` + `staleness`: freshness detection pipeline
- `main` → `collector` + `prompt`: documentation generation pipeline
- `main` → `template`: default template management
- `main` → `sanitize`: output size control
- `main` → `traverse`: project root detection
- `prompt` → `collector`: uses `SourceTree` for init prompts
- `prompt` → `staleness`: uses `StaleDoc` for update prompts
- `staleness` → `scanner`: uses `DocRefs` for mtime comparison
