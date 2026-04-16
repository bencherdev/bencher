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
- Use Astro slots and component props for dynamic content. Do not use `set:html` with string interpolation. Astro's native templating keeps content type-safe and eliminates XSS risk.
- When there are significant page-specific styles, extract them to a dedicated partial (e.g., `_pricing.scss`) in `src/styles/` and `@use` it from `styles.scss`.


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

## Playwright MCP Screenshots

When using the Playwright MCP tools, always write screenshots to the `.playwright-mcp/` directory at the repo root so they stay out of the working tree.

## Diagrams

Architecture and schema diagrams are defined as Mermaid `.mmd` files in `diagrams/` and pre-rendered to SVGs in `diagrams/output/` using `@mermaid-js/mermaid-cli`.

`npm run diagrams` is **not** run automatically by `npm run dev` or `npm run setup` because `mmdc` output is not deterministic and would produce noisy diffs on every dev startup. When modifying `.mmd` files, run `npm run diagrams` manually and commit the updated SVGs.

## Documentation

Available locally at [`services/console/src/content/`](./src/content/) or online at https://bencher.dev/docs/.

### i18n

When adding a new documentation update the chunks in all 9 language directories
(`en`, `de`, `es`, `fr`, `ja`, `ko`, `pt`, `ru`, `zh`)

### Docs Search (Pagefind)

Docs search uses [Pagefind](https://pagefind.app) and is **Bencher Cloud only**.
Pagefind runs as a post-build step against `dist/` and writes a static index into
`dist/pagefind/`. It is gated on `IS_BENCHER_CLOUD` so self-hosted builds skip it.

- In `npm run dev`, the search button renders but shows a stub message — the
  dev server does not build the Pagefind index.
- To exercise real search locally, build and serve:
  ```bash
  IS_BENCHER_CLOUD=true npm run build && npm run pagefind:node && npm run preview
  ```
- Pages are indexed via `data-pagefind-body` on the content column in
  [`src/layouts/docs/InnerLayout.astro`](./src/layouts/docs/InnerLayout.astro).
  Any page that does not go through `InnerLayout` is automatically excluded.

### Changelog

The changelog lives at [`services/console/src/chunks/docs-reference/changelog/en/changelog.mdx`](./src/chunks/docs-reference/changelog/en/changelog.mdx).
It is imported by the content pages in [`services/console/src/content/docs-reference/{lang}/changelog.mdx`](./src/content/docs-reference/{lang}/changelog.mdx).
