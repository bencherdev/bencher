#!/bin/bash

NAME=$1
VERSION=$2
ARCH=$3
DEB_DIR=$4
TRIPLE=${NAME}-${VERSION}-${ARCH}
DEB_PATH=$DEB_DIR/$TRIPLE
BIN_PATH=$DEB_PATH/usr/local/bin

mkdir -p $BIN_PATH
chmod 0755 $NAME
cp $NAME $BIN_PATH

DEBIAN_PATH=$DEB_PATH/DEBIAN
mkdir $DEBIAN_PATH
echo \
"Package: $NAME
Version: $VERSION
Architecture: $ARCH
Maintainer: Bencher <info@bencher.dev>
Description: Continuous Benchmarking" \
> $DEBIAN_PATH/control
cargo run --bin bencher --features docs -- docs --path $DEBIAN_PATH

dpkg-deb -Zxz --build --root-owner-group $DEB_PATH
