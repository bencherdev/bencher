#!/bin/bash

KIND=${1:-local}

docker compose -f docker/builder.docker-compose.yml build
docker compose -f docker/$KIND.docker-compose.yml up --build
