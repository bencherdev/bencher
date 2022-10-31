#!/bin/bash

./scripts/githooks.sh

curl -L https://fly.io/install.sh | sh
echo "export FLYCTL_INSTALL=\"/home/gitpod/.fly\"" >> $HOME/.bash_profile
echo "export PATH=\"/home/gitpod/.fly/bin:$PATH\"" >> $HOME/.bash_profile

rustup toolchain install nightly
rustup component add rustfmt
rustup component add clippy
cargo install cargo-udeps --locked

sudo apt-get update -q
sudo apt-get install -yq netcat-openbsd

echo "Waiting for vscode-sqltools"
until [ -d ~/.local/share/vscode-sqltools ]
do
    sleep .1
done

cd ~/.local/share/vscode-sqltools
npm install sqlite3
cd -

echo "Waiting for API server"
while ! nc -z localhost 61016; do
  sleep .1
done

cd ./tests/cli/rust_bench
./test.sh
cd -

source ~/.bash_profile
flyctl auth login
