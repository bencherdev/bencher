#!/usr/bin/env bash

dir=${PWD##*/}
if [ "$dir" == "tableflow" ]; then
    cd interpreter
elif [ "$dir" != "interpreter" ]; then
    cd ../interpreter
fi
wasm-pack build

cd ../client
yarn add ../interpreter/pkg/
gatsby develop
