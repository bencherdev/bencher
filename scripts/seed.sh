#!/bin/bash

cd services/cli
RUST_BACKTRACE=1 cargo test --features seed --test seed -- --nocapture
cd -

source ./scripts/mock.sh
