@echo off

call .\scripts\cli_env.bat

cd .\examples\rust\bench
bencher run "cargo bench"
