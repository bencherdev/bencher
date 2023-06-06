#!/bin/bash

# Add githooks
git config core.hooksPath .githooks

sudo apt-get update

# Install `mold`: https://github.com/rui314/mold
sudo apt-get install -y clang

MOLD_VERSION=1.11.0
MOLD_DEFAULT=true

echo "mold $MOLD_VERSION"
curl -L --retry 10 --silent --show-error https://github.com/rui314/mold/releases/download/v$MOLD_VERSION/mold-$MOLD_VERSION-$(uname -m)-linux.tar.gz | sudo tar -C /usr/local --strip-components=1 -xzf -
test $MOLD_DEFAULT = true -a "$(realpath /usr/bin/ld)" != /usr/local/bin/mold && sudo ln -sf /usr/local/bin/mold "$(realpath /usr/bin/ld)"; true

# Install `nodejs`: https://github.com/nodesource/distributions#debinstall
curl -fsSL https://deb.nodesource.com/setup_18.x | sudo -E bash - && \
sudo apt-get install -y nodejs

# Install `wasm-pack`: https://rustwasm.github.io/wasm-pack/installer/
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

# Install `sqlite3`: https://www.sqlite.org/index.html
sudo apt-get install -y sqlite3

# Install `flyctl`: https://fly.io/docs/getting-started/installing-flyctl/
curl -L https://fly.io/install.sh | sh
echo "export FLYCTL_INSTALL=\"/home/gitpod/.fly\"" >> $HOME/.bash_profile
echo "export PATH=\"/home/gitpod/.fly/bin:$PATH\"" >> $HOME/.bash_profile
source ~/.bash_profile

# Install rust tools
rustup self update
rustup update
rustup component add rust-src rustfmt clippy cargo
rustup target add wasm32-unknown-unknown
rustup toolchain install nightly
cargo install cargo-udeps --locked
cargo install diesel_cli --no-default-features --features sqlite --locked
cargo install typeshare-cli --locked
