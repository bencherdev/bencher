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

export ARCH=${1:-$(check_architecture)}
script_dir=`dirname $0`
docker compose -f $script_dir/docker-compose.yml up --build
