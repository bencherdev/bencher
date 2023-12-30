cd .\services\api
start /B cargo run

cd ..\cli
echo "Waiting for API server"
:while
  ping -n 5 127.0.0.1 >nul
  telnet localhost 61016 >nul 2>&1
  IF ERRORLEVEL 1 goto :while

set RUST_BACKTRACE="full"
cargo test --features seed --test seed -- --nocapture
