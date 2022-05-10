# https://hub.docker.com/_/rust
FROM rust:1.60.0-bullseye

RUN rustup toolchain install nightly
RUN rustup override set nightly

# Create util
WORKDIR /usr/src
RUN cargo new --lib util 
WORKDIR /usr/src/util
COPY functions/util/Cargo.toml Cargo.toml

# Create reports
WORKDIR /usr/src
RUN cargo new reports 
WORKDIR /usr/src/reports
COPY functions/reports/Cargo.toml Cargo.toml

# Cache all dependencies
RUN cargo test --no-run

# Copy over util code
WORKDIR /usr/src/util
COPY functions/util/src src

# Copy over reports code
WORKDIR /usr/src/reports
COPY functions/reports/src src

CMD cargo run