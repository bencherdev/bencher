#!/bin/bash
set -euo pipefail

RUST_BACKTRACE=1 cargo nextest run --all-features --no-capture --profile ci
RUST_BACKTRACE=1 cargo test --doc --all-features
