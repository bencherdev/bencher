NAME=$1
VERSION=$2
ARCH=$3
TRIPLE=${NAME}_${VERSION}_${ARCH}
DEB_PATH=deb/$TRIPLE
BIN_PATH=$DEB_PATH/usr/local/bin

mkdir -p $BIN_PATH
cp $NAME $BIN_PATH

DEBIAN_PATH=$DEB_PATH/DEBIAN
mkdir $DEBIAN_PATH
echo \
"Package: $NAME
Version: $VERSION
Architecture: $ARCH
Maintainer: Bencher <info@bencher.dev>
Description: Track your benchmarks. Catch performance regressions in CI." \
> $DEBIAN_PATH/control

dpkg-deb --build --root-owner-group $DEB_PATH