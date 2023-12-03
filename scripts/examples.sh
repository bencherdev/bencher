#!/bin/bash

source ./scripts/cli_env.sh

cd ./examples/rust/bench
bencher run "cargo bench"
