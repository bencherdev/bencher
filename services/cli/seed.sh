# run as `source ./seed.sh` or `. ./seed.sh`
RUST_BACKTRACE=1 cargo test --features seed --test seed -- --nocapture

export BENCHER_HOST=http://localhost:8080

LOGIN=$(cargo run -- auth login muriel.bagge@nowhere.com)
LOGIN_UUID=$(echo "$LOGIN" | sed -n -e 's/^.*"uuid": //p')
LOGIN_UUID=$(echo "$LOGIN_UUID" | tr -d '"')
echo $LOGIN_UUID
export BENCHER_TOKEN=$LOGIN_UUID

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

cargo run -- run --adapter rust "cargo bench"
