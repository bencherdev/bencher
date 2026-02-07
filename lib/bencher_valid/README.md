# bencher_valid

Input validation library for Bencher. Compiled to WASM for browser-side validation and used natively on the server.

## Feature Flags

This crate requires selecting a regex engine feature to compile:

- **`client`** — Uses `regex-lite` for a smaller binary (suitable for WASM/browser builds)
- **`server`** — Uses `regex` (full engine) and includes `rand` for server-side random generation

At least one of `client` or `server` must be enabled, otherwise types that depend on regex validation (e.g. `UserName`, and with `plus` enabled: `CardCvc`, `LastFour`, `CardNumber`) will fail to compile.

### Other Features

- **`schema`** — Derives `JsonSchema` (schemars) for all types
- **`db`** — Derives Diesel traits for database integration
- **`plus`** — Enables Bencher Plus (commercial) types such as payment card validation
- **`wasm`** — Enables WASM target support (implies `client`); includes `wasm-bindgen` and `console_error_panic_hook`
