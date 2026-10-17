#!/bin/bash

# Run as `source ./env.sh` or `. ./env.sh`

# Use the `--admin` flag to get an admin token
if [[ "$1" == "--admin" ]]; then
    # Valid until 2159-12-06T18:53:44Z
    export BENCHER_API_TOKEN=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJjbGllbnQiLCJleHAiOjU5OTM2NDM2MDksImlhdCI6MTY5ODY3NjMxNCwiaXNzIjoiaHR0cDovL2xvY2FsaG9zdDozMDAwLyIsInN1YiI6ImV1c3RhY2UuYmFnZ2VAbm93aGVyZS5jb20iLCJvcmciOm51bGx9.5krKtLMI4WezKfWICdy2u6t4FoGdq6bjgObjFzyYnE8
else
    # Valid until 2159-12-06T21:00:09Z
    export BENCHER_API_TOKEN=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJjbGllbnQiLCJleHAiOjU5OTM2MzYwMjQsImlhdCI6MTY5ODY2ODcyOSwiaXNzIjoiaHR0cDovL2xvY2FsaG9zdDozMDAwLyIsInN1YiI6Im11cmllbC5iYWdnZUBub3doZXJlLmNvbSIsIm9yZyI6bnVsbH0.wH9H44bRui5awHpj2Z0RYsp-6czbhQe6rr1vx_DPywA
fi

export BENCHER_HOST=http://localhost:6610
export BENCHER_PROJECT=the-computer
export BENCHER_BRANCH=master
export BENCHER_TESTBED=base
