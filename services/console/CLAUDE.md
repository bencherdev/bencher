# Console CLAUDE.md

The goal of this file is to describe the common mistakes and confusion points
an agent might face as they work in this codebase.
If you ever encounter something in the project that surprises you,
please alert the developer working with you and indicate that this is the case by editing the `CLAUDE.md` file to help prevent future agents from having the same issue.

## Project Overview

**Bencher Console** (`services/console`) - Web UI:
- TypeScript
- Astro
- SolidJS
- Bulma CSS

## Code Style

- Formatted and linted with Biome
- Use SolidJS patterns for reactivity
- Types are generated from Rust via typeshare - do not manually edit `src/types/bencher.ts`


## Building & Running

```bash
npm run dev
```

Runs at: http://localhost:3000

## Testing

```bash
&& npm test
```

## Formatting

```bash
npx biome format --write .
```

## Linting

```bash
npx biome lint .
```

## Console Setup

Runs Typeshare, WASM, and copies files to set up the console.

```bash
npm run setup
```

### Generate Types

```bash
npm run typeshare
```

### Generate WASM

```bash
npm run wasm
```

## Diagrams

Architecture and schema diagrams are defined as Mermaid `.mmd` files in `diagrams/` and pre-rendered to SVGs in `diagrams/output/` using `@mermaid-js/mermaid-cli`.

When modifying `.mmd` files, run `npm run diagrams` and commit the updated SVGs. The CI `Check Generated` job will fail if the committed SVGs are stale.

## Documentation

Available locally at [`services/console/src/content/`](./src/content/) or online at https://bencher.dev/docs/.

### i18n

When adding a new documentation update the chunks in all 9 language directories
(`en`, `de`, `es`, `fr`, `ja`, `ko`, `pt`, `ru`, `zh`)

### Changelog

The changelog lives at [`services/console/src/chunks/docs-reference/changelog/en/changelog.mdx`](./src/chunks/docs-reference/changelog/en/changelog.mdx).
It is imported by the content pages in [`services/console/src/content/docs-reference/{lang}/changelog.mdx`](./src/content/docs-reference/{lang}/changelog.mdx).
