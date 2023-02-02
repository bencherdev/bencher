#!/bin/bash

git push
git checkout main
git merge devel
git push
git checkout devel
