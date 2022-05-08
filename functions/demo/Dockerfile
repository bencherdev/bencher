# https://hub.docker.com/_/rust
FROM rust:1.60.0-buster

RUN rustup toolchain install nightly
RUN rustup override set nightly
WORKDIR /usr/src
RUN cargo new demo
WORKDIR /usr/src/demo
COPY functions/demo/Cargo.toml Cargo.toml
RUN cargo test --no-run
COPY functions/demo/src src

EXPOSE 8080
CMD cargo run
