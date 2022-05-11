# https://hub.docker.com/_/rust
FROM rust:1.60.0-bullseye

RUN rustup toolchain install nightly
RUN rustup override set nightly

# Create util
WORKDIR /usr/src/lib
RUN cargo new reports
WORKDIR /usr/src/lib/reports
COPY lib/reports/Cargo.toml Cargo.toml

# Create util
WORKDIR /usr/src/functions
RUN cargo new util 
WORKDIR /usr/src/functions/util
COPY functions/util/Cargo.toml Cargo.toml

# Create reports
WORKDIR /usr/src/functions
RUN cargo new --lib reports 
WORKDIR /usr/src/functions/reports
COPY functions/reports/Cargo.toml Cargo.toml
RUN mkdir /usr/src/functions/reports/src/bin
RUN echo "fn main() {}" > /usr/src/functions/reports/src/bin/fn_reports.rs

# Cache all dependencies
RUN cargo test --no-run


# Copy over lib code
WORKDIR /usr/src/lib/reports
COPY lib/reports/src src

# Copy over util code
WORKDIR /usr/src/functions/util
COPY functions/util/src src

# Copy over reports code
WORKDIR /usr/src/functions/reports
COPY functions/reports/src src

CMD cargo run