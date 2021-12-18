# Rollback

rustup target add wasm32-unknown-unknown

cargo install --locked cargo-watch
cargo install --locked cargo-edit --features vendored-openssl
cargo install --locked cargo-udeps
cargo install --locked cargo-audit

cargo install --locked trunk