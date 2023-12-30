cd .\services\api
START /B cargo run

cd ..\cli
echo "Waiting for API server"
:while
  timeout 1
  if ! telnet localhost 61016 >nul 2>&1 goto while

RUST_BACKTRACE=full cargo test --features seed --test seed -- --nocapture
