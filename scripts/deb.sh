VERSION=$1
ARCH=$2
TRIPLE=bencher_${VERSION}_${ARCH}
DEB_PATH=deb/$TRIPLE

cd ./services/cli
cargo build --release --target x86_64-unknown-linux-gnu

cd ../..
BIN_PATH=$DEB_PATH/usr/local/bin
mkdir -p $BIN_PATH
cp target/release/bencher $BIN_PATH

DEBIAN_PATH=$DEB_PATH/DEBIAN
mkdir $DEBIAN_PATH
echo \
"Package: bencher
Version: $VERSION
Architecture: $ARCH
Maintainer: Bencher <info@bencher.dev>
Description: Track your benchmarks. Catch performance regressions in CI." \
> $DEBIAN_PATH/control

dpkg-deb --build --root-owner-group $DEB_PATH