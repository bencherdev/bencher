#!/bin/bash

# run as `source ./env.sh` or `. ./env.sh`

# Use the `--admin` flag to get an admin token
if [[ "$1" == "--admin" ]]; then
    # Valid until 2159-12-06T18:53:44Z
    export BENCHER_API_TOKEN=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjo1OTkzNjQzNjA5LCJpYXQiOjE2OTg2NzYzMTQsImlzcyI6Imh0dHA6Ly9sb2NhbGhvc3Q6MzAwMC8iLCJzdWIiOiJldXN0YWNlLmJhZ2dlQG5vd2hlcmUuY29tIiwib3JnIjpudWxsfQ.xumYID-R4waqhyjhcbSlwartbiRJ2AwngVkevLUBVCA
else
    # Valid until 2159-12-06T21:00:09Z
    export BENCHER_API_TOKEN=eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhcGlfa2V5IiwiZXhwIjo1OTkzNjM2MDI0LCJpYXQiOjE2OTg2Njg3MjksImlzcyI6Imh0dHA6Ly9sb2NhbGhvc3Q6MzAwMC8iLCJzdWIiOiJtdXJpZWwuYmFnZ2VAbm93aGVyZS5jb20iLCJvcmciOm51bGx9.t3t23mlgKYZmUt7-PbRWLqXlCTt6Ydh8TRE8KiSGQi4
fi

export BENCHER_HOST=http://localhost:61016
export BENCHER_PROJECT=the-computer
export BENCHER_BRANCH=master
export BENCHER_TESTBED=base
