#!/bin/bash

docker compose -f builder.docker-compose.yml build
docker compose -f local.docker-compose.yml up --build -d
