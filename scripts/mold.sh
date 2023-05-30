#!/bin/bash

sudo apt-get update
sudo apt-get install -y clang

MOLD_VERSION=1.11.0
MOLD_DEFAULT=true

echo "mold $MOLD_VERSION"
curl -L --retry 10 --silent --show-error https://github.com/rui314/mold/releases/download/v$MOLD_VERSION/mold-$MOLD_VERSION-$(uname -m)-linux.tar.gz | sudo tar -C /usr/local --strip-components=1 -xzf -
test $MOLD_DEFAULT = true -a "$(realpath /usr/bin/ld)" != /usr/local/bin/mold && sudo ln -sf /usr/local/bin/mold "$(realpath /usr/bin/ld)"; true
