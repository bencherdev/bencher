# Bencher 

rustup target add wasm32-unknown-unknown

cargo install --locked cargo-watch
cargo install --locked cargo-edit --features vendored-openssl
cargo install --locked cargo-udeps --features vendored-openssl
cargo install --locked cargo-audit --features vendored-openssl

cargo install --locked trunk

cargo install --locked wasm-pack

cd bencher
cargo run -- -x "cargo bench" --url http://localhost:8080/ repo --url git@github.com:epompeii/bencher_db.git --key $HOME/.ssh/id_ed25519 --push

cargo run --bin bencher -- ...

cd ui
wasm-pack build

cd www
npm run start

