# https://hub.docker.com/_/rust
FROM rust:1.65.0-bullseye

WORKDIR /usr/src
COPY Cargo.toml Cargo.toml

WORKDIR /usr/src/lib
COPY lib/bencher_adapter bencher_adapter
COPY lib/bencher_json bencher_json
COPY lib/bencher_rbac bencher_rbac
COPY lib/bencher_valid bencher_valid

WORKDIR /usr/src/services
RUN cargo init cli

WORKDIR /usr/src/services/api
COPY services/api/src src
COPY services/api/Cargo.toml Cargo.toml
COPY services/api/migrations migrations
COPY services/api/diesel.toml diesel.toml

RUN cargo build --release
