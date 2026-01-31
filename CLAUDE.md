# CLAUDE.md

This file provides guidance for AI assistants (like Claude) when working with the Bencher codebase.

## Project Overview

Bencher is a continuous benchmarking platform that helps detect and prevent performance regressions. It consists of:

- **`bencher` CLI** (`services/cli`) - Command-line tool for running benchmarks and interacting with the API
- **Bencher API Server** (`services/api`) - REST API backend built with Rust
- **Bencher Console** (`services/console`) - Web UI built with Astro + SolidJS

## Tech Stack

- **Backend**: Rust (edition 2024, toolchain 1.91.1)
- **Frontend**: TypeScript, Astro, SolidJS, Bulma CSS
- **Database**: SQLite (via Diesel ORM)
- **WASM**: Used for sharing Rust types with the frontend
- **CI/CD**: GitHub Actions
- **Version Control**: Jujutsu (`jj`) with Git

## Repository Structure

```
services/
  api/          # Rust API server
  cli/          # Rust CLI (bencher command)
  console/      # Astro + SolidJS web UI
lib/
  api_*/        # API endpoint handlers
  bencher_*/    # Shared Rust libraries
plus/           # Bencher Plus (commercial) features
tasks/          # Build tasks (test_api, gen_types, etc.)
xtask/          # Cargo xtask runner
```

## Common Commands

### Building

```bash
cargo build                    # Build all Rust crates
cargo build --release          # Release build
```

### Running the Development Environment

```bash
# Terminal 1: Run the API server
cd services/api
cargo run

# Terminal 2: Run the Console
cd services/console
npm run dev
```

The console is accessible at http://localhost:3000 and the API at http://localhost:61016.

### Testing

```bash
cargo test                     # Run all Rust tests
cargo test-api seed            # Seed the database with sample data
cd services/console && npm test # Run frontend tests
```

### Linting & Formatting

```bash
# Rust
cargo fmt                      # Format Rust code
cargo clippy --no-deps --all-features -- -Dwarnings  # Lint Rust code

# Frontend (console)
cd services/console
npm run fmt                    # Format with Biome
npm run lint                   # Lint with Biome
```

### Type Generation

```bash
cargo gen-types                # Generate OpenAPI schema and TypeScript types from Rust
cd services/console
npm run typeshare              # Generate TypeScript types from Rust
npm run wasm                   # Build WASM packages
npm run setup                  # Run typeshare + wasm + copy files
```

## Code Style Guidelines

### Rust

- Use `#[expect(...)]` instead of `#[allow(...)]` for lint suppression
- Avoid `unwrap()` and `expect()` in production code (allowed in tests)
- Avoid unbounded channels - use bounded `mpsc::channel` instead
- Avoid `select!` macros - use `futures_concurrency::stream::Merge::merge`
- Maximum cognitive complexity: 25
- Use absolute paths sparingly (max 3 segments, diesel crate exempt)

### Frontend (TypeScript)

- Formatted and linted with Biome
- Use SolidJS patterns for reactivity
- Types are generated from Rust via typeshare

## Feature Flags

The codebase uses feature flags extensively:

- `plus` - Enables Bencher Plus (commercial) features
- `sentry` - Enables Sentry error tracking
- `otel` - Enables OpenTelemetry observability

Default builds include all features. To build without Plus features:

```bash
cargo build --no-default-features
```

## API Documentation

The API uses Dropshot and generates an OpenAPI spec at `services/api/openapi.json`.

## Database

SQLite database located at `services/api/data/bencher.db` for testing. Access via:

```bash
sqlite3 services/api/data/bencher.db
```

## Docker

```bash
docker/run.sh                  # Build and run with Docker
# Or manually:
ARCH=arm64 docker compose --file docker/docker-compose.yml up --build
```

## Key Libraries

- `bencher_adapter` - Benchmark harness adapters (parsing benchmark output)
- `bencher_json` - JSON types shared across the codebase
- `bencher_client` - Generated API client
- `bencher_boundary` - Statistical analysis for threshold detection
- `bencher_valid` - Input validation types

## Bencher Documentation

Documentation about how to use Bencher is available locally at `services/console/src/content/`
or online at https://bencher.dev/docs/.

## Notes for AI Assistants

1. **Workspace Structure**: This is a Cargo workspace with many crates. Changes often span multiple crates.
2. **Type Sharing**: Rust types are shared with TypeScript via `typeshare`. After modifying types in Rust, run `npm run typeshare` in the console directory.
3. **API Changes**: The API uses Dropshot. OpenAPI spec is generated and stored at `services/api/openapi.json`.
4. **Strict Linting**: The project has extensive Clippy lints enabled. Run `cargo clippy` to check for issues.
5. **Plus Features**: Some features are gated behind the `plus` feature flag for the commercial version.
