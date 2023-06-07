#!/bin/bash

git config core.hooksPath .githooks

cd ./services/action
npm install --include=dev
