#!/bin/bash

sudo apt-get update
sudo apt-get install -y clang

MOLD=mold-1.11.0-x86_64-linux

curl -L $MOLD.tar.gz | sudo tar -C /opt -xz

echo 'PATH="$PATH:/opt/$MOLD/bin"' >> ~/.profile