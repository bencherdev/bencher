#!/bin/bash

./scripts/githooks.sh

sudo apt-get update -q
sudo apt-get install -yq netcat

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
