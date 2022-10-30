#!/bin/bash

TAG=$(./scripts/version.sh)
git tag $TAG
git push origin $TAG
