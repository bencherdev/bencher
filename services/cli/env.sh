#!/bin/bash

# Run as `source ./env.sh` or `. ./env.sh`
# Set `BENCHER_API_KEY` to a user API key first: sign in to the Console at
# http://localhost:3000 and make one under your account's Keys.
if [[ -z "${BENCHER_API_KEY:-}" ]]; then
    echo "Set BENCHER_API_KEY to a user API key made in the Console" >&2
fi

export BENCHER_HOST=http://localhost:6610
export BENCHER_PROJECT=the-computer
export BENCHER_BRANCH=master
export BENCHER_TESTBED=base
