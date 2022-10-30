#!/bin/bash

cat /workspace/bencher.json | sed -E 's/"/\\"/g;'
