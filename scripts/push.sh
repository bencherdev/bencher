#!/bin/bash

git push
git checkout cloud
git merge devel
git push
git checkout devel
