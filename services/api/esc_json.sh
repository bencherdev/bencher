#!/bin/bash

cat $1 | sed -E 's/"/\\"/g;'
