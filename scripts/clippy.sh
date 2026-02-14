#!/bin/bash
set -euo pipefail

cargo clippy --no-deps --all-targets --all-features -- -Dwarnings
