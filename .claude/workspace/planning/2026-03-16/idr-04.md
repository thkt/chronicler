# IDR: テンプレートに analyzer agent の解析技法を吸収

> 2026-03-16

## Summary

`/docs` スキルとanalyzer agent（architecture/api/domain/setup）をchroniclerに完全移行するにあたり、各analyzerが持っていたフレームワーク検出・スキーマ探索・ツール固有の解析戦略をテンプレートの `## Analysis Techniques` セクションとして吸収した。

## Changes

### [src/templates/architecture.md](file:////Users/thkt/GitHub/chronicler/src/templates/architecture.md)

- `## Analysis Techniques` セクション追加: version detection（.nvmrc等）、tree-sitter or Grep fallback、dependency enumeration（jq / Cargo.toml）、import graph構築

### [src/templates/api.md](file:////Users/thkt/GitHub/chronicler/src/templates/api.md)

- `## Analysis Techniques` セクション追加: framework detection、route discovery（Next.js/Express/FastAPI別パターン）、schema discovery、auth detection、route-schema correlation

### [src/templates/domain.md](file:////Users/thkt/GitHub/chronicler/src/templates/domain.md)

- `## Analysis Techniques` セクション追加: ORM detection（Prisma/TypeORM/Sequelize等）、exhaustive field extraction、nullable detectionルール、domain logic discovery globパターン

### [src/templates/setup.md](file:////Users/thkt/GitHub/chronicler/src/templates/setup.md)

- `## Analysis Techniques` セクション追加: package manager detection、env var discovery + Zod cross-validationルール、config deep read対象、script discovery

## Decisions

- analyzer agentの知見をテンプレートの指示文として吸収する方式を採用。Claudeが自力で解析する際のガイダンスとなる
- analyzer agent自体は `.claude/` から削除（Trashへ移動）。chroniclerのテンプレートが代替する
