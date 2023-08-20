#!/bin/bash

KIND=${1:-local}

cd docker
docker compose -f new.builder.docker-compose.yml build
docker compose -f new.docker-compose.yml up --build
# docker compose -f $KIND.docker-compose.yml up --build -d
cd -
