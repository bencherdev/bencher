cd .\services\api
START /B cargo run

cd ..\cli
echo "Waiting for API server"
:while
  ping -n 1 127.0.0.1 >NUL
  telnet localhost 61016 >nul 2>&1
  IF ERRORLEVEL 1 goto :while

RUST_BACKTRACE=full cargo test --features seed --test seed -- --nocapture
