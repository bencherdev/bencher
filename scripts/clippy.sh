#!/bin/bash

cargo clippy --no-deps --all-targets --all-features -- -Dwarnings
