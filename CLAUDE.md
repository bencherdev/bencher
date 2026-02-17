# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Bencher is a continuous benchmarking platform that detects and prevents performance regressions. It consists of:

- **Bencher API Server** (`services/api`) - REST API backend built with Rust/Dropshot
- **Bencher Console** (`services/console`) - Web UI built with Astro + SolidJS + Bulma CSS
- **`bencher` CLI** (`services/cli`) - Command-line tool for running benchmarks and interacting with the API

**Tech stack:** Rust (edition 2024, toolchain 1.91.1), TypeScript, SQLite (Diesel ORM), WASM for sharing Rust types with frontend. Version control uses Jujutsu (`jj`) with Git.

**Development Methodology:** Practice test-driven development (TDD) with a strong emphasis on code quality, maintainability, and clear documentation. Follow the established code style rules and project architecture to ensure consistency across the codebase.

## Common Commands

### Building & Running

```bash
cargo build                    # Build all Rust crates
# API server:
cd services/api && cargo run   # Runs on http://localhost:61016
# Console:
cd services/console && npm run dev  # Runs on http://localhost:3000
```

### Testing

```bash
cargo nextest run              # Run all Rust tests
cargo nextest run -p <crate_name>  # Run tests for a specific crate
cargo test --doc               # Run doctests (nextest doesn't support doctests)
cargo test-api seed            # Seed the database with sample data
cd services/console && npm test # Run frontend tests (vitest)
```

Crates that depend on `bencher_valid` will need to specify either:

1. `server` feature for server-side usage
2. `client` feature for client-side usage

Otherwise, you will see:

```
use of undeclared type `Regex`
```

Running the seed tests:

1. Stop the API server if it's running.
2. Delete the existing database at `services/api/data/bencher.db` if it exists.
3. Run the API server from `services/api` in one terminal.
4. In another terminal:
  - Bencher Cloud: `cargo test-api seed --is-bencher-cloud`
  - Bencher Self-Hosted: `cargo test-api seed`
  - If running in a separate `jj` workspace (no `.git` directory), add `--no-git`. The anonymous report tests derive the project name from the git repo; without git they fall back to `"Project"` instead of `"bencher"`, and the `--no-git` flag adjusts assertions accordingly.

### Linting & Formatting

```bash
cargo fmt                      # Format Rust code
cargo clippy --no-deps --all-targets --all-features -- -Dwarnings  # Lint Rust code
cargo check --no-default-features  # Verify build without Plus features

cd services/console
npx biome format --write .     # Format frontend code
npx biome lint .               # Lint frontend code
```

### Type Generation (run after API changes)

```bash
cargo gen-types                # Generate OpenAPI spec + TypeScript types
# Or individually:
cargo gen-spec                 # Generate only OpenAPI spec (services/api/openapi.json)
cargo gen-ts                   # Generate only TypeScript types
```

### Console Setup

```bash
cd services/console
npm run typeshare              # Generate TypeScript types from Rust structs
npm run wasm                   # Build WASM packages (bencher_valid)
npm run setup                  # Run typeshare + wasm + copy files
```

## Architecture

### Repository Structure

```
services/
  api/            # Rust API server (Dropshot)
  cli/            # Rust CLI
  console/        # Astro + SolidJS web UI
lib/
  api_auth/       # Authentication endpoints
  api_checkout/   # Billing endpoints
  api_organizations/ # Organization endpoints
  api_projects/   # Project endpoints
  api_run/        # Benchmark run endpoints
  api_server/     # Server config endpoints
  api_users/      # User endpoints
  bencher_adapter/   # Benchmark harness output parsers
  bencher_boundary/  # Statistical threshold detection
  bencher_client/    # Generated API client (from OpenAPI via progenitor)
  bencher_json/      # Shared JSON types (typeshare annotated)
  bencher_schema/    # Database schema, models, migrations (Diesel)
  bencher_valid/     # Input validation (compiled to WASM for frontend)
plus/             # Bencher Plus (commercial) features
  api_oci/           # OCI Distribution Spec registry endpoints
  api_runners/       # Runner management and agent endpoints (Plus)
  bencher_oci_storage/ # OCI blob/manifest storage (S3-backed)
  bencher_otel/      # OpenTelemetry instrumentation (Plus)
tasks/            # Build tasks invoked via cargo aliases
xtask/            # Administrative tasks (local only, not CI)
```

### Type Sharing Flow

Rust is the single source of truth for types:
1. Rust structs annotated with `#[typeshare]` in `bencher_json` and other crates
2. `cargo gen-types` generates OpenAPI spec (`services/api/openapi.json`) and TypeScript types (`services/console/src/types/bencher.ts`)
3. `bencher_valid` is compiled to WASM for browser-side validation
4. `bencher_client` is auto-generated from the OpenAPI spec via progenitor

### Database Access Macros

Three macros control database access patterns:
- `public_conn!()` - Read-only public access (optionally takes a `PublicUser`)
- `auth_conn!()` - Read-only authenticated access
- `write_conn!()` - Single writer access

All have single-use and expanded closure-like forms for multiple uses in the same scope.

### Cargo Aliases

Defined in `.cargo/config.toml`:
- `cargo xtask` - Administrative tasks
- `cargo gen-types` / `cargo gen-spec` / `cargo gen-ts` - Type generation
- `cargo test-api` - API testing and DB seeding
- `cargo test-runner` - Runner integration tests (requires Linux + KVM; see [`services/runner/CLAUDE.md`](services/runner/CLAUDE.md))
- `cargo bin-version` - Version management

### Feature Flags

- `plus` - Bencher Plus (commercial) features
- `sentry` - Sentry error tracking
- `otel` - OpenTelemetry observability

Default builds include all features. Build without: `cargo build --no-default-features`

### OCI Registry (Plus Feature)

The API server includes an OCI Distribution Spec compliant container registry, registered at `/v2/` paths. Key crates:
- `plus/api_oci` - Dropshot endpoints for blobs, manifests, tags, referrers, and uploads
- `plus/bencher_oci_storage` - Storage backend using S3 (via Access Points)
- OCI auth tokens use a dedicated `Oci` audience in `bencher_token`
- Registry is configured via `JsonRegistry` in the server config (`plus.registry`)
- Run the OCI conformance tests with `cargo test-api oci --admin`

## Code Style Rules

### Rust

- Always run `cargo fmt` and `cargo clippy` when testing or before committing
- Run `cargo fmt` one final time after all changes are complete (including any generated code or lint fixes), since clippy fixes and other automated changes can introduce formatting drift
- Use `#[expect(...)]` instead of `#[allow(...)]` for lint suppression
- Do **NOT** suppress a lint outside of a test module without explicit approval
- All dependency versions go in the workspace `Cargo.toml`
- When reviewing code, also check:
  - `cargo check --no-default-features`
  - `cargo gen-types` (if the API changed at all)
- Use idiomatic, strong types instead of `String` and `serde_json::Value` where possible
- Database model fields should use strong validated types (e.g., `ProjectId`, `ProjectUuid`, `ProjectName`, `DateTime`, `VersionNumber`) with Diesel `ToSql`/`FromSql` impls rather than raw primitives (`i32`, `i64`, `String`). All conversion happens inside the Diesel impls, not in the model layer.
- Avoid `select!` macros - use `futures_concurrency::stream::Merge::merge`
- All time-based tests should be deterministic and use time manipulation not real wall-clock time
- Use `bencher_json::Clock::Custom` (behind the `test-clock` feature) to inject a fake clock in tests instead of calling `DateTime::now()` directly. `Clock` is available on `ApiContext`.
- Most wire type definitions are in the `bencher_valid` or `bencher_json` crate
- Always pass strong types (`MyTypeId`, `MyTypeUuid`, etc) into a function instead of its stringly typed equivalent, even in tests
- Do **NOT** use shared, global mutable state
- Always prefer to use `thiserror` for error types
- Do not use `Box<dyn Error>` (or `Box<dyn std::error::Error + Send + Sync>`) as a return type. Use `HttpError` for API endpoint errors or define specific `thiserror` error enums. The only acceptable uses of `Box<dyn Error>` are when wrapping third-party APIs that return boxed errors (e.g., diesel migrations, dropshot server creation).
- Do **NOT** use `dyn std::any::Any` without explicit justification and approval
- When adding workspace dependencies without extra options (no `optional`, no `features`), use the shorthand `dep.workspace = true` form instead of `dep = { workspace = true }`
- Use `camino` (`Utf8Path`/`Utf8PathBuf`) for file paths whenever practical instead of `std::path::Path`/`PathBuf`
- Use `clap` for CLI argument parsing
  - The `clap` struct definitions should live in a separate `parser` module
  - The subcommand handler logic should live in a separate module named after the binary for production code (ie `bencher`) or a module named `task` for `tasks/*` crates

### Frontend (TypeScript)

- Formatted and linted with Biome
- Use SolidJS patterns for reactivity
- Types are generated from Rust via typeshare - do not manually edit `src/types/bencher.ts`

### Scripts Policy

Shell scripts are used very sparingly. Prefer creating tasks in `tasks/` (invoked via cargo aliases). Administrative-only tasks go in `xtask/`. Shell scripts are only acceptable as ultra-lightweight wrappers around commands like `git` or `docker`.

## Git Flow

- PRs are opened against the `devel` branch
- Deploy to Bencher Cloud: reset `cloud` to `devel` and push
- After successful deploy: CI resets `main` to `cloud`
- Release tags (e.g., `v0.5.10`) are created off `devel`

## Database

SQLite database at `services/api/data/bencher.db`. Migrations live in `lib/bencher_schema/migrations/`. If a migration is already in the `cloud` branch, it has been applied to production.

## Database Access

In the Rust code there are three macros used to access the database:

1. `public_conn!()` - For read-only public access
   1. This optionally takes in a `PublicUser`
2. `auth_conn!()` - For read-only authenticated access
3. `write_conn!()` - For single writer access

All of these macros have a single-use and expanded closure-like form for use multiple times in the same scope.

## Docker

When adding a new crate, update both Dockerfiles:
- `services/api/Dockerfile`
- `services/console/Dockerfile`

## Runner System (Plus Feature)

Bare metal benchmark runners that claim and execute jobs from the API. Server-scoped (runners serve all projects).

**Key concepts:**
- **Runner**: A registered agent authenticated via `bencher_runner_`-prefixed tokens (SHA-256 hashed in DB)
- **Job**: Linked to a report, follows state machine: `pending → claimed → running → completed/failed/canceled`
- **Claim endpoint**: Long-poll POST that atomically claims pending jobs (priority DESC, FIFO within tier)
- **WebSocket channel**: Persistent connection for heartbeat and status updates during job execution

**Key files:**
- `plus/api_runners/` - Runner CRUD, token rotation, job claiming, job updates, WebSocket channel
- `lib/bencher_json/src/runner/` - Shared JSON types (`JsonRunner`, `JsonJob`, `JobStatus`)
- `lib/bencher_schema/src/model/runner/` - Database models and queries
- `lib/api_projects/src/jobs.rs` - Project-scoped job listing (public API)
- `lib/bencher_schema/migrations/2026-02-02-120000_runner/` - Migration for `runner` and `job` tables

**Runner authentication** is separate from user auth — runner tokens use `Authorization: Bearer bencher_runner_<token>` and are validated by hashing and looking up `token_hash` in the `runner` table.

**Runner integration tests** require Linux + KVM and run on a remote GCP VM. See [`services/runner/CLAUDE.md`](services/runner/CLAUDE.md) for the full guide on connecting, transferring code, and running `cargo test-runner scenarios`.

## Key Coordination Points

Changes in these areas often require updates across multiple files:
- **API endpoints** → run `cargo gen-types` → commit updated `openapi.json` + `bencher.ts`
- **Rust types with `#[typeshare]`** → run `npm run typeshare` in console
- **New crate** → update workspace `Cargo.toml` + both Dockerfiles
- **Validation types** → may need WASM rebuild (`npm run wasm` in console)
- **Runner endpoints** → behind `plus` feature flag; runner model in `bencher_schema` also gated on `plus`

## Documentation

Available locally at `services/console/src/content/` or online at https://bencher.dev/docs/.

See [`services/console/CLAUDE.md`](services/console/CLAUDE.md) for console-specific guides, including how to add API documentation pages.

### Server Configuration Documentation

Server config types live in `lib/bencher_json/src/system/config/`. The corresponding documentation chunks live in `services/console/src/chunks/docs-reference/server-config/`.

When adding a new config field:
1. Update the Rust type in `lib/bencher_json/src/system/config/`
2. Update the documentation chunks in all 9 language directories (`en`, `de`, `es`, `fr`, `ja`, `ko`, `pt`, `ru`, `zh`)
3. Update the example config in `services/console/src/chunks/docs-reference/server-config/example.mdx`
4. If adding a new subsection under `plus`, create a new `plus-<name>.mdx` chunk in all 9 languages and import/render it in each language's `plus.mdx`
