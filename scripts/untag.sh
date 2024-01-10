#!/bin/bash

VERSION=$(cargo xtask version)
TAG="v$VERSION"
git tag -d $TAG
git push --delete origin $TAG
