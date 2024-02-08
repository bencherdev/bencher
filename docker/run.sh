#!/bin/bash

function check_architecture() {
    local arch=$(uname -m)
    if [[ "$arch" == "x86_64" ]]; then
        echo "x86_64"
    elif [[ "$arch" == "aarch64" ]] || [[ "$arch" == "arm64" ]]; then
        echo "arm64"
    else
        exit 1
    fi
}

ARCH=${1:-$(check_architecture)}

docker compose -f docker/docker-compose.$ARCH.yml up --build
