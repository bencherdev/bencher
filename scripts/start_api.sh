#!/bin/bash

rustup self update
rustup update
rustup component add rust-src
rustup component add rustfmt
rustup component add clippy
cargo run
