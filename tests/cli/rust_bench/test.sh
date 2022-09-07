cd ../../../services/cli

# run as `source ./seed.sh` or `. ./seed.sh`
RUST_BACKTRACE=1 cargo test --features seed --test seed -- --nocapture

export BENCHER_HOST=http://localhost:8080

# Valid until 2027-09-05T19:03:59Z
export BENCHER_API_TOKEN=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjoxODIwMTcxMDM5LCJpYXQiOjE2NjIzODY0MDksImlzcyI6ImJlbmNoZXIuZGV2Iiwic3ViIjoibXVyaWVsLmJhZ2dlQG5vd2hlcmUuY29tIn0.sfAJmF9qIl_QRNnh8uLYuODHnxufXt_3m7skcNp1kMs

BRANCH=$(cargo run -- branch view --project the-computer master)
BRANCH_UUID=$(echo "$BRANCH" | sed -n -e 's/^.*"uuid": //p')
BRANCH_UUID=$(echo "$BRANCH_UUID" | tr -d '"')
echo $BRANCH_UUID
export BENCHER_BRANCH=$BRANCH_UUID

TESTBED=$(cargo run -- testbed view --project the-computer base)
TESTBED_UUID=$(echo "$TESTBED" | sed -n -e 's/^.*"uuid": //p')
TESTBED_UUID=$(echo "$TESTBED_UUID" | tr -d '"')
echo $TESTBED_UUID
export BENCHER_TESTBED=$TESTBED_UUID

cd ../../tests/cli/rust_bench

../../../target/debug/bencher run --adapter rust_bench --iter 3 "cargo +nightly bench"
