@echo off

cd services\api
start cmd /k cargo run

cd ..\cli
echo "Waiting for API server"

:while
powershell -Command "if (Test-NetConnection -ComputerName localhost -Port 61016) { exit } else { Start-Sleep -Seconds 1 }"
if %errorlevel% neq 0 goto while

set RUST_BACKTRACE=full
cargo test --features seed --test seed -- --nocapture
