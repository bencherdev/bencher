#!/bin/bash

VERSION=$(cat Cargo.toml | sed -n -e 's/^version = //p' | tr -d '"')
echo "v$VERSION"
