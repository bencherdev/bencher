# https://hub.docker.com/_/rust
FROM rust:1.60.0-bullseye

RUN rustup toolchain install nightly
RUN rustup override set nightly
WORKDIR /usr/src
RUN cargo new reports 
WORKDIR /usr/src/reports
COPY functions/reports/Cargo.toml Cargo.toml
RUN cargo test --no-run
COPY functions/reports/src src

CMD cargo run