#!/bin/bash

cat Cargo.toml | sed -n -e 's/^version = //p' | tr -d '"'
