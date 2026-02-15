# Console CLAUDE.md

## Adding API Documentation

API docs pages are `.mdx` files in `src/content/api-{collection}/` directories. Astro's content layer auto-discovers them — no menu, routing, or sidebar config changes are needed.

### Collections

| Collection | Directory | URL prefix |
|---|---|---|
| `api_organizations` | `src/content/api-organizations/` | `/docs/api/organizations/{slug}/` |
| `api_projects` | `src/content/api-projects/` | `/docs/api/projects/{slug}/` |
| `api_users` | `src/content/api-users/` | `/docs/api/users/{slug}/` |
| `api_server` | `src/content/api-server/` | `/docs/api/server/{slug}/` |

### Steps to add a new API docs page

1. **Create the `.mdx` file** in the appropriate `src/content/api-{collection}/` directory.

2. **Use this frontmatter template:**

```mdx
---
title: "Page Title"
description: "The Bencher ... REST API"
heading: "... REST API"
sortOrder: N
paths:
  - path: /v0/the/api/path
    method: get
    headers: pub
    cli: subcommand list RESOURCE
  - path: /v0/the/api/path/{id}
    method: get
    headers: pub
    cli: subcommand view RESOURCE ID
---
```

3. **Bump `sortOrder`** in sibling files if inserting between existing entries. Every file in the same collection must have a unique `sortOrder` — it controls sidebar ordering.

4. **Ensure the endpoints exist in the OpenAPI spec** (`services/api/openapi.json` and `public/download/openapi.json`). The `Operation.astro` component reads endpoint details (summary, description, parameters, request body) from `public/download/openapi.json`. If the path/method pair is missing from the spec, the page renders but the endpoint details are empty. Run `cargo gen-types` if needed, and `npm run copy` (or `npm run setup`) to sync the spec into `public/download/`.

### Frontmatter fields

| Field | Required | Description |
|---|---|---|
| `title` | yes | Page title, shown in sidebar menu |
| `description` | yes | Meta description for SEO |
| `heading` | yes | H1 heading rendered on the page |
| `sortOrder` | yes | Integer controlling sidebar position (ascending) |
| `paths` | yes | Array of API operations to render |

### Path entry fields

| Field | Required | Values | Description |
|---|---|---|---|
| `path` | yes | e.g. `/v0/projects/{project}/jobs` | Must match a key in `openapi.json` `paths` |
| `method` | yes | `get`, `post`, `put`, `patch`, `delete` | Must match the HTTP method under the path |
| `headers` | yes | `pub`, `auth`, `img` | `pub` = public (optional auth), `auth` = required auth, `img` = image endpoint |
| `cli` | no | e.g. `job list PROJECT` or `null` | Bencher CLI command shown on the page. Set to `null` or omit if no CLI equivalent |

### How it works (key files)

- `src/content.config.ts` — Defines the `api()` collection with a Zod schema and glob loader (`*.mdx`)
- `src/components/docs/menu/ApiList.astro` — Renders the sidebar menu by loading all entries from each API collection via `getEnOnlyCollection()`, sorted by `sortOrder`
- `src/pages/docs/api/{collection}/[slug].astro` — Dynamic route that generates a page per entry via `getStaticPaths()`
- `src/components/docs/api/Operation.astro` — Renders each endpoint by looking up `path`+`method` in `public/download/openapi.json`
- `src/i18n/utils.ts` — `getEnOnlyCollection()` loads and sorts entries; `getEnOnlyPaths()` generates static paths

### Verification

Run `cd services/console && npm run dev` and navigate to the new page URL. Confirm:
- The page renders with endpoint details (summary, parameters, etc.)
- The page title appears in the API sidebar under the correct collection heading
- The sidebar ordering matches the intended `sortOrder` position
