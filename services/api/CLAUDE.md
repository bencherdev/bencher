# API Server CLAUDE.md

The goal of this file is to describe the common mistakes and confusion points
an agent might face as they work in this codebase.
If you ever encounter something in the project that surprises you,
please alert the developer working with you and indicate that this is the case by editing the `CLAUDE.md` file to help prevent future agents from having the same issue.

## Project Overview

**Bencher API Server** (`services/api`) - REST API backend:
- Rust
- Dropshot
- Diesel
- SQLite
- Litestream

## Building & Running

```bash
cargo run
```

Runs at: http://localhost:61016

## Testing

Running the seed tests:

1. Stop the API server if it's running.
2. Delete the existing database at `services/api/data/bencher.db` if it exists.
3. Run the API server from `services/api` in one terminal.
4. In another terminal:
  - Bencher Cloud: `cargo test-api seed --is-bencher-cloud`
  - Bencher Self-Hosted: `cargo test-api seed`
  - If running in a separate `jj` workspace (no `.git` directory), add `--no-git`. The anonymous report tests derive the project name from the git repo; without git they fall back to `"Project"` instead of `"bencher"`, and the `--no-git` flag adjusts assertions accordingly.

## Generate Types

Always run after API changes!
Generates OpenAPI spec and TypeScript types for the console.

```bash
cargo gen-types
```

### Generate OpenAPI Spec

```bash
cargo gen-spec
```

Outputs to: [`services/api/openapi.json`](./openapi.json)

### Generate TypeScript Types

```bash
cargo gen-ts
```

Outputs to: [`services/console/src/types/bencher.ts`](../console/src/types/bencher.ts)

## Database

SQLite database at `services/api/data/bencher.db`. Migrations live in `lib/bencher_schema/migrations/`. If a migration is already in the `cloud` branch, it has been applied to production.

### Database Access Macros

Three macros control database access patterns:
- `public_conn!()` - Read-only public access (optionally takes a `PublicUser`)
- `auth_conn!()` - Read-only authenticated access
- `write_conn!()` - Single writer access

All have single-use and expanded closure-like forms for multiple uses in the same scope.

If performing multiple database writes in a row, create a Diesel transaction with the `write_conn!()` macro.

## Feature Flags

- `plus` - Bencher Plus (commercial) features
- `sentry` - Sentry error tracking
- `otel` - OpenTelemetry observability

Default builds include all features. Build without: `cargo build --no-default-features`

## OCI Registry (Plus Feature)

The API server includes an OCI Distribution Spec compliant container registry,
registered at `/v2/` paths.
