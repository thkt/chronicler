# Architecture Documentation

Generate an architecture overview for a developer joining this project.

## Sections

### 1. Technology Stack

Table with columns: Category | Technology | Version

Include: language, framework, runtime, database (omit rows that don't apply).

### 2. Directory Structure

Project tree output. Exclude: .git, node_modules, target, **pycache**, dist, build.

### 3. Module Structure

Mermaid `graph TD` diagram showing module/component relationships.
Use descriptive node IDs and quote labels containing `/` or `.`:

```mermaid
graph TD
    main["src/main.rs"] --> config["src/config.rs"]
```

### 4. Data Flow

Describe the path of a typical request or operation from entry point to output.
Use a Mermaid `sequenceDiagram` if the flow involves multiple components.

Example: for a CLI tool, trace a command from argument parsing to output.
For a web app, trace an HTTP request from route handler to response.

### 5. Key Components

Table with columns: Component | Path | Description

List the main modules, entry points, and core abstractions.
Use `file_path:line_number` references in the Path column.

### 6. Dependencies

**External**: list each package/crate/library with its purpose.

**Internal**: describe how modules depend on each other.
Format: `module_a → module_b: relationship description`

## Analysis Techniques

1. **Version detection**: read `.nvmrc`, `.python-version`, `.ruby-version`, `rust-toolchain.toml`, `package.json` engines field
2. **Directory structure**: use `tree -L 3` or Glob to map the project layout
3. **Code structure**: if `tree-sitter-analyzer` is available, use `tree-sitter-analyzer {file} --structure` for precise extraction. Otherwise, use Grep to find module definitions, exports, and import/use statements
4. **Dependency enumeration**: parse `package.json` dependencies with jq, or read `Cargo.toml` / `go.mod` / `pyproject.toml` directly
5. **Import graph**: Grep for `import`/`use`/`require` statements to build the internal dependency map

## Writing Guidelines

- Write for a developer joining the project for the first time
- Explain "why" each component exists, not just "what" it is
- Use `file_path:line_number` references to link to source code
- Keep descriptions concise — one sentence per component
- When updating, verify each `file_path:line_number` reference is still accurate

## Omit Rules

- Omit a section only if fewer than 2 items would appear
- Never omit sections 1–4 (Technology Stack, Directory Structure, Module Structure, Data Flow)
