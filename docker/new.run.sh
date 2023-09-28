#!/bin/bash

KIND=${1:-local}

cd docker
docker compose -f new.builder.docker-compose.yml build
docker compose -f new.$KIND.docker-compose.yml up --build
cd -
