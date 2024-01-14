#!/bin/bash

# TODO user per-target profiles
# In the mean time, we can use sed to change the value of wasm-opt on aaarch64
# https://github.com/rust-lang/cargo/issues/4897

CARGO_TOML="../../lib/bencher_valid/Cargo.toml"
ARCH=$(uname -m)
WASM_OPT="wasm-opt ="

test $ARCH = "aarch64" && sed -i "s/$WASM_OPT true/$WASM_OPT false/g" $CARGO_TOML; true

wasm-pack build ../../lib/bencher_valid --target web --no-default-features --features plus,wasm

test $ARCH = "aarch64" && sed -i "s/$WASM_OPT false/$WASM_OPT true/g" $CARGO_TOML; true
