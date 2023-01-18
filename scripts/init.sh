#!/bin/bash

./scripts/githooks.sh

sudo apt-get update -q
sudo apt-get install -yq netcat-openbsd sqlite3

rustup target add wasm32-unknown-unknown
rustup toolchain install nightly
cargo install cargo-udeps --locked
cargo install diesel_cli --no-default-features --features sqlite --locked
cargo install --path services/cli --locked

curl -L https://fly.io/install.sh | sh
echo "export FLYCTL_INSTALL=\"/home/gitpod/.fly\"" >> $HOME/.bash_profile
echo "export PATH=\"/home/gitpod/.fly/bin:$PATH\"" >> $HOME/.bash_profile

source ~/.bash_profile
