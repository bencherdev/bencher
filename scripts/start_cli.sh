#!/bin/bash

rustup component add cargo
cargo install --path . --locked

echo "Waiting for API server"
while ! nc -z localhost 61016; do
  sleep 1
done

RUST_BACKTRACE=1 cargo test --features seed --test seed -- --nocapture

../../scripts/mock.sh
