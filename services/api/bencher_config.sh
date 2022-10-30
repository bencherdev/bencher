#!/bin/bash

# Run with `. ./bencher_config.sh`
export BENCHER_CONFIG=$(cat $1)
echo $BENCHER_CONFIG
