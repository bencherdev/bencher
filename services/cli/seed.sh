# run as `source ./seed.sh` or `. ./seed.sh`
cargo test --features seed --test seed

LOGIN=$(cargo run -- auth login --host http://localhost:8080 muriel.bagge@nowhere.com)
LOGIN_UUID=$(echo "$LOGIN" | sed -n -e 's/^.*"uuid": //p')
LOGIN_UUID=$(echo "$LOGIN_UUID" | tr -d '"')
echo $LOGIN_UUID
export BENCHER_TOKEN=$LOGIN_UUID

BRANCH=$(cargo run -- branch view --host http://localhost:8080 --project the-computer master)
BRANCH_UUID=$(echo "$BRANCH" | sed -n -e 's/^.*"uuid": //p')
BRANCH_UUID=$(echo "$BRANCH_UUID" | tr -d '"')
echo $BRANCH_UUID
export BENCHER_BRANCH=$BRANCH_UUID

TESTBED=$(cargo run -- testbed view --host http://localhost:8080 --project the-computer base)
TESTBED_UUID=$(echo "$TESTBED" | sed -n -e 's/^.*"uuid": //p')
TESTBED_UUID=$(echo "$TESTBED_UUID" | tr -d '"')
echo $TESTBED_UUID
export BENCHER_TESTBED=$TESTBED_UUID

cargo run -- run --host http://localhost:8080 --adapter rust "cargo bench"
