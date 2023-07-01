#!/bin/bash

RUST_BACKTRACE=1 cargo test --features seed --test seed -- --nocapture

source ../../scripts/mock.sh
