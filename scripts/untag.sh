#!/bin/bash

VERSION=$(cargo bin-version)
TAG="v$VERSION"
git tag -d $TAG
git push --delete origin $TAG
