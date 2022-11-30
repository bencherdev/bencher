#!/bin/bash

npm install --include=dev

echo "Waiting for API server"
while ! nc -z localhost 61016; do
  sleep 1
done

npm run dev
