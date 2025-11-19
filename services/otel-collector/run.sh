#!/bin/sh

docker build --tag otel-collector .
docker run --name otel-collector --rm -p 4318:4318 otel-collector
