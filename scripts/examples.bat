@echo off

call .\scripts\cli_env.bat

cd .\examples\rust\bench
powershell -Command "bencher run ""cargo bench"""
