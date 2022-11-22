#!/bin/bash

echo "Waiting for API server"
while ! nc -z localhost 61016; do
  sleep .1
done

cd ./tests/cli/rust_bench
./test.sh
cd -
