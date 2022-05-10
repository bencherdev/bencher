# https://hub.docker.com/_/rust
FROM rust:1.60.0-bullseye

RUN rustup toolchain install nightly
WORKDIR /usr/src
RUN cargo new reports 
WORKDIR /usr/src/reports
COPY reports/Cargo.toml Cargo.toml
RUN cargo test --no-run
COPY reports/src src

CMD cargo +nightly run