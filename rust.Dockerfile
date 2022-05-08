# https://hub.docker.com/_/rust
FROM rust:1.60.0-buster

RUN rustup toolchain install nightly
RUN rustup override set nightly
WORKDIR /usr/src
RUN cargo new reports
WORKDIR /usr/src/reports
COPY reports/Cargo.toml Cargo.toml
RUN cargo test --no-run
WORKDIR /usr/src
RUN cargo new bencher
WORKDIR /usr/src/bencher
COPY bencher/Cargo.toml Cargo.toml
RUN cargo test --no-run
COPY reports/src reports/src
COPY bencher/src bencher/src
