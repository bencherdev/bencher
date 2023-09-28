#!/bin/bash

KIND=${1:-local}

cd docker
docker compose -f builder.docker-compose.yml build
docker compose -f $KIND.docker-compose.yml up --build
cd -
