# Bencher CLAUDE.md

The goal of this file is to describe the common mistakes and confusion points
an agent might face as they work in this codebase.
If you ever encounter something in the project that surprises you,
please alert the developer working with you and indicate that this is the case by editing the `CLAUDE.md` file to help prevent future agents from having the same issue.

## Project Overview

Bencher is a continuous benchmarking platform that detects and prevents performance regressions.

- **Bencher API Server** (`services/api`) - see [`services/api/CLAUDE.md`](services/api/CLAUDE.md)
- **`bencher` CLI** (`services/cli`) - CLI for the REST API
- **Bencher Console** (`services/console`) - see [`services/console/CLAUDE.md`](services/console/CLAUDE.md)
- **Bare Metal `runner`** (`services/runner`) - Bare Metal benchmark runner

## Version Control

Version control uses Jujutsu (`jj`) with Git.

## Development Methodology

- Practice test-driven development (TDD)
- All new code should be designed for testability and maintainability
- All changes should include appropriate unit and integration tests

## Building

```bash
cargo build
```

## Testing

```bash
cargo nextest run
```

`nextest` does not support doctests, so also run:

```bash
cargo test --doc
```

Crates that depend on `bencher_valid` will need to specify either:

1. `server` feature for server-side usage
2. `client` feature for client-side usage

Otherwise, you will see:

```
use of undeclared type `Regex`
```

## Formatting

```bash
cargo fmt
```

## Linting

```bash
cargo clippy --no-deps --all-targets --all-features -- -Dwarnings
```

## Checking non-Plus

```bash
cargo check --no-default-features
```

## Linux Cross-Compilation Checks

When modifying `target_os = "linux"` crates (`bencher_init`, `bencher_rootfs`, `bencher_runner`, `bencher_runner_cli`),
also run the cross-compilation checks locally:

```bash
./scripts/clippy.sh            # Runs clippy natively + cross-compiles to x86_64-unknown-linux-gnu
./scripts/test.sh --linux-only # Cross-compiles tests for the Linux-only crates
```

These scripts require a cross-compiler (`zig`, `x86_64-linux-gnu-gcc`, or `x86_64-unknown-linux-gnu-gcc`)
and the `x86_64-unknown-linux-gnu` Rust target.
The clippy script will install the target automatically and warn if no cross-compiler is found.

## Code Quality

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
- Always use `thiserror` for error types in libraries and production binaries (`services/`). Do not use `anyhow` in those crates. `anyhow` is acceptable in `tasks/` crates (build tasks, test harnesses) where convenience outweighs structured errors.
- Do not use `Box<dyn Error>` (or `Box<dyn std::error::Error + Send + Sync>`) as a return type. Use `HttpError` for API endpoint errors or define specific `thiserror` error enums. The only acceptable uses of `Box<dyn Error>` are when wrapping third-party APIs that return boxed errors (e.g., diesel migrations, dropshot server creation).
- Do **NOT** use `dyn std::any::Any` without explicit justification and approval
- When adding workspace dependencies without extra options (no `optional`, no `features`), use the shorthand `dep.workspace = true` form instead of `dep = { workspace = true }`
- Use `camino` (`Utf8Path`/`Utf8PathBuf`) for file paths whenever practical instead of `std::path::Path`/`PathBuf`. Exception: `tempfile::tempdir()` in tests may use `std::path` since it returns `TempDir` with `&Path`; convert via `Utf8Path::from_path()` at the boundary when needed.
- Use `clap` for CLI argument parsing
  - The `clap` struct definitions should live in a separate `parser` module
  - The subcommand handler logic should live in a separate module named after the binary for production code (ie `bencher`) or a module named `task` for `tasks/*` crates
  - Do **NOT** use `num_args` on flags in `bencher run` — it uses `trailing_var_arg = true` to match `docker run` semantics, and `num_args` conflicts with trailing vararg parsing. Validate collection sizes at the type/deserialization layer instead (e.g., `TryFrom` impls in `bencher_json`).

## Scripts Policy

Shell scripts are used very sparingly. Prefer creating tasks in `tasks/` (invoked via cargo aliases). Administrative-only tasks go in `xtask/`. Shell scripts are only acceptable as ultra-lightweight wrappers around commands like `git` or `docker`.

### Cargo Aliases

Defined in `.cargo/config.toml`:
- `cargo xtask` - Administrative tasks
- `cargo gen-types` / `cargo gen-spec` / `cargo gen-ts` - Type generation
- `cargo test-api` - API testing and DB seeding
- `cargo test-runner` - Runner integration tests (requires Linux + KVM)

## Git Flow

- PRs are opened against the `devel` branch
- Deploy to Bencher Cloud: reset `cloud` to `devel` and push
- After successful deploy: CI resets `main` to `cloud`
- Release tags (e.g., `v0.5.10`) are created off `devel`

### Type Sharing Flow

Rust is the single source of truth for types:
1. Rust structs annotated with `#[typeshare]` in `bencher_json` and other crates
2. `cargo gen-types` generates OpenAPI spec (`services/api/openapi.json`) and TypeScript types (`services/console/src/types/bencher.ts`)
3. `bencher_valid` is compiled to WASM for browser-side validation
4. `bencher_client` is auto-generated from the OpenAPI spec via progenitor

## Docker

When adding a new crate, update both `Dockerfile`s:
- [`services/api/Dockerfile`](./services/api/Dockerfile)
- [`services/console/Dockerfile`](./services/console/Dockerfile)
