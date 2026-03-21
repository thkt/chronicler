# chronicler - Setup Guide

## Prerequisites

| Tool  | Version               | Required |
| ----- | --------------------- | -------- |
| Rust  | 2024 edition (stable) | Yes      |
| Cargo | (bundled with Rust)   | Yes      |

## Installation

### Homebrew (recommended)

```bash
brew install thkt/tap/chronicler
```

### From Release Binary

```bash
# macOS (Apple Silicon)
curl -L https://github.com/thkt/chronicler/releases/latest/download/chronicler-aarch64-apple-darwin.tar.gz | tar xz
mv chronicler ~/.local/bin/
```

### From Source

```bash
git clone https://github.com/thkt/chronicler.git
cd chronicler
cargo build --release
cp target/release/chronicler ~/.local/bin/
```

## Configuration

**Config Files**:

| File                 | Purpose                                              |
| -------------------- | ---------------------------------------------------- |
| `.claude/tools.json` | Per-project chronicler settings (`src/config.rs:4`)  |
| `hooks/hooks.json`   | Claude Code hook registration (`hooks/hooks.json:1`) |

**Settings** (in `.claude/tools.json` under `chronicler` key):

| Setting   | Description                             | Required | Default                   | Source             |
| --------- | --------------------------------------- | -------- | ------------------------- | ------------------ |
| dir       | Documentation directory to scan         | No       | `workspace/docs`          | `src/config.rs:38` |
| templates | Template directory path                 | No       | `workspace/doc-templates` | `src/config.rs:39` |
| edit      | Enable PostToolUse edit detection       | No       | `true`                    | `src/config.rs:40` |
| stop      | Enable Stop hook check                  | No       | `true`                    | `src/config.rs:41` |
| mode      | `warn` (advisory) or `block` (blocking) | No       | `warn`                    | `src/config.rs:42` |

## Running

### As a Claude Code Hook

Add to `~/.claude/settings.json` or install via plugin:

```bash
claude plugins marketplace add github:thkt/chronicler
claude plugins install chronicler
```

### Direct Execution

```bash
# Check mode (freshness check)
chronicler check .

# Init mode (generate init prompt)
chronicler init .

# Update mode (generate update prompt)
chronicler update .

# Edit mode (pipe hook JSON)
echo '{"tool_input":{"file_path":"/project/src/auth.ts"}}' | chronicler edit
```

## Testing

- **Full suite**: `cargo test`
- **Single module**: `cargo test collector::` or `cargo test template::`
- **Single test**: `cargo test t_001_collects_source_files`

## Common Workflows

| Task                   | Command                 |
| ---------------------- | ----------------------- |
| Run tests              | `cargo test`            |
| Release build          | `cargo build --release` |
| Format code            | `cargo fmt`             |
| Check without building | `cargo check`           |
