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

## Replication (`plus/bencher_replica` and Litestream)

`plus/bencher_replica` is the in-process replacement for Litestream: it ships WAL segments to a file or S3 replica, snapshots via a single-step `SQLite` online backup, and restores at startup. Configure via `plus.replica` in the server config; when BOTH `plus.litestream` and `plus.replica` are set, the replica runs in shadow mode (Litestream keeps checkpoint ownership and remains the restore source). The crate's `src/lib.rs` documents the six invariants (I1 to I6) that govern every design decision; read them before touching the sync engine. Its tests require `--features plus,testing`.

## Litestream

Litestream (v0.5.x, LTX-based) runs as a child process of the API server and is the sole WAL checkpointer (`wal_autocheckpoint = 0`).

- When Litestream decides it needs a full re-snapshot (WAL continuity break, salt reset, or process restart), it copies the entire database into a local LTX file while holding the SQLite write lock via its `_litestream_lock` table. While that copy runs, API writes cannot take the write lock: each one burns its `busy_timeout` and fails with "database is locked" (surfaced as retryable 503s plus Sentry tripwires). The copy scales with database size, so the larger the database the longer the stall; removing this blocking failure mode is the reason `plus/bencher_replica` exists. A burst of "database is locked" Conflicts arriving together with a large read-plus-write burst on the data volume points at a Litestream re-snapshot, not the `/v0/server/backup` endpoint.
- `truncate-page-n: 0` genuinely disables blocking TRUNCATE checkpoints in v0.5.x (`TruncatePageN` is a `*int` in Litestream's config, so explicit 0 is honored, verified against the v0.5.13 source).
- `JsonLitestream.metrics_port` enables Litestream's Prometheus endpoint (serialized as the top-level `addr` setting in `litestream.yml`). The Fly configs (`services/api/fly/*.toml`) scrape port 9090 via their `[[metrics]]` section.

## Building & Running

```bash
cargo run
```

Runs at: http://localhost:6610

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

## MCP Server (Plus Feature)

The API server includes a stateless MCP (Model Context Protocol) server
(`plus/api_mcp`), registered at `POST /mcp` (`unpublished`, so it does not
appear in the OpenAPI spec). On Bencher Cloud it is fronted by
`mcp.bencher.dev`, a DNS alias for the API server; routing is purely path-based.

Its tools call the same `pub` `*_inner` functions as the REST endpoints in
`lib/api_projects` and `lib/api_run`, and the tool surface deliberately mirrors
what a project API key (`bencher_run_`) can do.

### MCP Sync

When changing any `ApiActor`-accepting endpoint (its params, semantics, or the
set of operations a `bencher_run_` key can perform), also update:

- The corresponding tool in `plus/api_mcp/src/tools/`
- The Bencher skill docs: `skills/bencher/mcp.md`

MCP tool input types must not be recursive: tool schemas are generated with
`schemars` `inline_subschemas`, which recurses without bound, so a recursive
input type would overflow the stack at schema generation time.
