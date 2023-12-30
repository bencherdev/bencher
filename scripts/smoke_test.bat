@echo off

cd services\api
start /B cargo run

cd ..\cli
echo "Waiting for API server"

:while
powershell -Command "$connection = Test-NetConnection -ComputerName localhost -Port 61016; if ($connection.TcpTestSucceeded) { exit 0 } else { exit 1 }"
if %errorlevel% neq 0 goto while

set RUST_BACKTRACE=full
cargo test --features seed --test seed -- --nocapture
