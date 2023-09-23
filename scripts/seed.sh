#!/bin/bash

RUST_BACKTRACE=full cargo test --features seed --test seed -- --nocapture

source ../../scripts/mock.sh
