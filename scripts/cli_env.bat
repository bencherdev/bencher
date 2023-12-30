@REM  run as `call .\cli_env.bat`

@REM Use the `--admin` flag to get an admin token
IF "%1" == "--admin" (
    @REM Valid until 2159-12-06T18:53:44Z
    set BENCHER_API_TOKEN="eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjo1OTkzNjQzNjA5LCJpYXQiOjE2OTg2NzYzMTQsImlzcyI6Imh0dHA6Ly9sb2NhbGhvc3Q6MzAwMC8iLCJzdWIiOiJldXN0YWNlLmJhZ2dlQG5vd2hlcmUuY29tIiwib3JnIjpudWxsfQ.xumYID-R4waqhyjhcbSlwartbiRJ2AwngVkevLUBVCA"
) ELSE (
    @REM Valid until 2159-12-06T21:00:09Z
    set BENCHER_API_TOKEN="eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjo1OTkzNjM2MDI0LCJpYXQiOjE2OTg2Njg3MjksImlzcyI6Imh0dHA6Ly9sb2NhbGhvc3Q6MzAwMC8iLCJzdWIiOiJtdXJpZWwuYmFnZ2VAbm93aGVyZS5jb20iLCJvcmciOm51bGx9.t3t23mlgKYZmUt7-PbRWLqXlCTt6Ydh8TRE8KiSGQi4"
)

set BENCHER_HOST="http://localhost:61016"
set BENCHER_PROJECT="the-computer"
set BENCHER_BRANCH="master"
set BENCHER_TESTBED="base"
