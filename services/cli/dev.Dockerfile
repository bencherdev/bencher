# https://hub.docker.com/_/rust
FROM rust:1.60.0-bullseye

RUN rustup toolchain install nightly
RUN rustup override set nightly
WORKDIR /usr/src
RUN cargo new reports
WORKDIR /usr/src/reports
COPY lib/reports/Cargo.toml Cargo.toml
RUN cargo test --no-run
WORKDIR /usr/src
RUN cargo new cli
WORKDIR /usr/src/cli
COPY cli/Cargo.toml Cargo.toml
RUN cargo test --no-run
COPY lib/reports/src reports/src
COPY cli/src bencher/src
