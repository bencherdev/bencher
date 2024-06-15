# https://mcr.microsoft.com/v2/vscode/devcontainers/rust/tags/list
FROM mcr.microsoft.com/vscode/devcontainers/rust:1.0.1-1-bookworm

RUN sudo apt-get update

# Install `nc`
RUN sudo apt-get install -y netcat-openbsd

# Install `mold`: https://github.com/rui314/mold
ARG MOLD_VERSION
ARG MOLD_DEFAULT

RUN sudo apt-get install -y clang
RUN echo "mold $MOLD_VERSION"
RUN curl -L --retry 10 --silent --show-error https://github.com/rui314/mold/releases/download/v$MOLD_VERSION/mold-$MOLD_VERSION-$(uname -m)-linux.tar.gz | sudo tar -C /usr/local --strip-components=1 -xzf -
RUN test $MOLD_DEFAULT = true -a "$(realpath /usr/bin/ld)" != /usr/local/bin/mold && sudo ln -sf /usr/local/bin/mold "$(realpath /usr/bin/ld)"; true

# Install `nodejs`: https://github.com/nodesource/distributions#debinstall
RUN curl -fsSL https://deb.nodesource.com/setup_18.x | sudo -E bash - && \
    sudo apt-get install -y nodejs

# Install `biome`: https://biomejs.dev/guides/getting-started/
RUN sudo npm install -g --save-exact @biomejs/biome@1.7.3

# Install `@esbuild/linux-arm64` and `@rometools/cli-linux-arm64` for `aarch64`
RUN test $(uname -m) = "aarch64" && sudo npm install -g @esbuild/linux-arm64 @rometools/cli-linux-arm64; true

# Install `lychee` dependencies: https://github.com/lycheeverse/lychee
RUN sudo apt-get install -y gcc pkg-config libc6-dev libssl-dev

# Install `sqlite3`: https://www.sqlite.org/index.html
RUN sudo apt-get install -y sqlite3

# Install `plotters` dependencies: https://github.com/plotters-rs/plotters#ubuntu-linux
RUN sudo apt-get install -y pkg-config libfreetype6-dev libfontconfig1-dev

# Install as `vscode`
USER vscode

# Install Rust tools
ARG RUST_VERSION

RUN rustup self update
RUN rustup update
RUN rustup toolchain install $RUST_VERSION-$(uname -m)-unknown-linux-gnu
RUN rustup component add clippy rust-src rustfmt --toolchain $RUST_VERSION-$(uname -m)-unknown-linux-gnu
RUN rustup target add wasm32-unknown-unknown --toolchain $RUST_VERSION-$(uname -m)-unknown-linux-gnu
RUN cargo install diesel_cli --version 2.1.1 --locked --no-default-features --features sqlite
RUN cargo install wasm-pack --version 0.12.1 --locked
RUN cargo install typeshare-cli --version 1.9.2 --locked
# RUN cargo install lychee --version 0.15.0 --locked

# Install `flyctl`: https://fly.io/docs/getting-started/installing-flyctl/
RUN curl -L https://fly.io/install.sh | sh
RUN echo "export FLYCTL_INSTALL=\"/workspaces/bencher/.fly\"" >> $HOME/.bash_profile
RUN echo "export PATH=\"/workspaces/bencher/.fly/bin:$PATH\"" >> $HOME/.bash_profile

# Install `litestream`: https://litestream.io/install/debian/
ARG LITESTREAM_VERSION
ARG LITESTREAM_ARCH
RUN sudo wget https://github.com/benbjohnson/litestream/releases/download/v${LITESTREAM_VERSION}/litestream-v${LITESTREAM_VERSION}-linux-${LITESTREAM_ARCH}.deb
RUN sudo dpkg -i litestream-v${LITESTREAM_VERSION}-linux-${LITESTREAM_ARCH}.deb
