#!/bin/bash

./scripts/githooks.sh

sudo apt-get update -q
sudo apt-get install -yq netcat-openbsd sqlite3

rustup toolchain install nightly
rustup comonent add rust-src
rustup component add rustfmt
rustup component add clippy
cargo install cargo-udeps --locked
cargo install diesel_cli --no-default-features --features sqlite --locked

echo "Waiting for API server"
while ! nc -z localhost 61016; do
  sleep .1
done

cd ./tests/cli/rust_bench
./test.sh
cd -

curl -L https://fly.io/install.sh | sh
echo "export FLYCTL_INSTALL=\"/home/gitpod/.fly\"" >> $HOME/.bash_profile
echo "export PATH=\"/home/gitpod/.fly/bin:$PATH\"" >> $HOME/.bash_profile

source ~/.bash_profile
flyctl auth login
