#!/bin/bash

TAG="$(./scripts/v.sh)"
git tag -d $TAG
git push --delete origin $TAG
