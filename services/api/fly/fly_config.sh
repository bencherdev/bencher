#!/bin/bash

flyctl secrets set BENCHER_CONFIG="$(cat $1)"
