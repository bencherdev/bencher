# Build Stage
# https://hub.docker.com/_/rust
FROM rust:1.64.0-bullseye as builder

WORKDIR /usr/src/lib
COPY lib/bencher_json bencher_json
COPY lib/bencher_rbac bencher_rbac

WORKDIR /usr/src/api
COPY api/src src
COPY api/Cargo.toml Cargo.toml
COPY api/migrations migrations
COPY api/diesel.toml diesel.toml

RUN cargo build --release
