# https://hub.docker.com/_/rust
FROM rust:1.70.0-bullseye

RUN apt-get update && \
    apt-get install -y clang

ARG MOLD_VERSION
RUN curl -L --retry 10 --silent --show-error https://github.com/rui314/mold/releases/download/v${MOLD_VERSION}/mold-${MOLD_VERSION}-$(uname -m)-linux.tar.gz | tar -C /usr/local --strip-components=1 -xzf -
RUN "$(realpath /usr/bin/ld)" != /usr/local/bin/mold && sudo ln -sf /usr/local/bin/mold "$(realpath /usr/bin/ld)"; true

WORKDIR /usr/src
COPY Cargo.toml Cargo.toml

WORKDIR /usr/src/.cargo
COPY .cargo/config.toml config.toml

WORKDIR /usr/src/lib
COPY lib/bencher_adapter bencher_adapter
COPY lib/bencher_json bencher_json
COPY lib/bencher_plot bencher_plot
COPY lib/bencher_rbac bencher_rbac
COPY lib/bencher_valid bencher_valid

WORKDIR /usr/src/plus
COPY plus/bencher_billing bencher_billing
COPY plus/bencher_license bencher_license
COPY plus/bencher_plus bencher_plus

WORKDIR /usr/src
RUN cargo init xtask

WORKDIR /usr/src/services
RUN cargo init cli

WORKDIR /usr/src/services/api
COPY services/api/src src
COPY services/api/Cargo.toml Cargo.toml
COPY services/api/migrations migrations
COPY services/api/diesel.toml diesel.toml

RUN cargo build --release
