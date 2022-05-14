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
WORKDIR /usr/src/api
RUN cargo new util 
WORKDIR /usr/src/api/util
COPY api/util/Cargo.toml Cargo.toml

# Create reports
WORKDIR /usr/src/api
RUN cargo new --lib reports 
WORKDIR /usr/src/api/reports
COPY api/reports/Cargo.toml Cargo.toml
RUN mkdir /usr/src/api/reports/src/bin
RUN echo "fn main() {}" > /usr/src/api/reports/src/bin/fn_reports.rs

# Cache all dependencies
RUN cargo test --no-run


# Copy over lib code
WORKDIR /usr/src/lib/reports
COPY lib/reports/src src

# Copy over util code
WORKDIR /usr/src/api/util
COPY api/util/src src

# Copy over reports code
WORKDIR /usr/src/api/reports
COPY api/reports/src src

CMD cargo run