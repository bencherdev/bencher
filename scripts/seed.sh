#!/bin/bash

RUST_BACKTRACE=full cargo test --features seed --test seed -- --nocapture

# run as `source ./seed.sh` or `. ./seed.sh`
export BENCHER_HOST=http://localhost:61016
# Valid until 2027-09-05T19:03:59Z
export BENCHER_API_TOKEN=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjoxODIwMTcxMDM5LCJpYXQiOjE2NjIzODY0MDksImlzcyI6ImJlbmNoZXIuZGV2Iiwic3ViIjoibXVyaWVsLmJhZ2dlQG5vd2hlcmUuY29tIn0.sfAJmF9qIl_QRNnh8uLYuODHnxufXt_3m7skcNp1kMs
export BENCHER_PROJECT=the-computer
export BENCHER_BRANCH=master
export BENCHER_TESTBED=base
