#!/bin/bash

ARCH=${1:-$(uname -m)}

docker compose -f docker/docker-compose.$ARCH.yml up --build

# docker compose -f docker/builder.docker-compose.yml build
# docker compose -f docker/$KIND.docker-compose.yml up --build
