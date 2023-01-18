#!/bin/bash

rustup self update
rustup update
rustup component add rust-src
rustup component add rustfmt
rustup component add clippy
rustup component add cargo
rustup target add wasm32-unknown-unknown
rustup toolchain install nightly
cargo install cargo-udeps --locked
cargo install diesel_cli --no-default-features --features sqlite --locked
