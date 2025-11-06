#!/bin/bash

# TODO use per-target profiles
# In the mean time, we can use sed to change the value of wasm-opt on aaarch64
# https://github.com/rust-lang/cargo/issues/4897

CARGO_TOML="../../lib/bencher_valid/Cargo.toml"
ARCH=$(uname -m)
WASM_OPT="wasm-opt ="

PLUS=",plus"
if [[ "$IS_BENCHER_PLUS" == "false" ]]; then
    echo "Building without \`plus\` feature"
    PLUS=""
fi

test $ARCH = "aarch64" && sed -i "s/$WASM_OPT true/$WASM_OPT false/g" $CARGO_TOML; true

wasm-pack build ../../lib/bencher_valid --target web --no-default-features --features wasm$PLUS

# wasm-pack copies over the LICENSE.md file
# https://github.com/rustwasm/wasm-pack/issues/407
rm ../../lib/LICENSE.md

test $ARCH = "aarch64" && sed -i "s/$WASM_OPT false/$WASM_OPT true/g" $CARGO_TOML; true
