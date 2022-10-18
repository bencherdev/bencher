# Build Stage
# https://hub.docker.com/_/rust
FROM rust:1.64.0-bullseye as builder

WORKDIR /usr/src
COPY Cargo.toml Cargo.toml

WORKDIR /usr/src/services
RUN cargo init cli

WORKDIR /usr/src/services/lib
COPY services/lib/bencher_json bencher_json
COPY services/lib/bencher_rbac bencher_rbac

WORKDIR /usr/src/services/api
COPY services/api/src src
COPY services/api/Cargo.toml Cargo.toml
COPY services/api/migrations migrations
COPY services/api/diesel.toml diesel.toml

RUN cargo build --release
