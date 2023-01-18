#!/bin/bash

echo "Waiting for API server"
while ! nc -z localhost 61016; do
  sleep 1
done

RUST_BACKTRACE=1 cargo test --features seed --test seed -- --nocapture

cargo install --path . --locked

source ../../scripts/mock.sh
