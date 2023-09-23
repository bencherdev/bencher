#!/bin/bash

cd ./services/api
cargo run &

cd ../cli
echo "Waiting for API server"
while ! nc -z localhost 61016; do
  sleep 1
done

RUST_BACKTRACE=full cargo test --features seed --test seed -- --nocapture

cargo install --path . --locked

source ../../scripts/mock.sh
