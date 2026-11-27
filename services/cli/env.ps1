# Run as `.\env.ps1`
# Set `BENCHER_API_KEY` to a user API key first: sign in to the Console at
# http://localhost:3000 and make one under your account's Keys.
if (-not $env:BENCHER_API_KEY) {
    Write-Warning "Set BENCHER_API_KEY to a user API key made in the Console"
}

$env:BENCHER_HOST="http://localhost:6610"
$env:BENCHER_PROJECT="the-computer"
$env:BENCHER_BRANCH="master"
$env:BENCHER_TESTBED="base"
