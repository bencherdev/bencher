# https://hub.docker.com/_/rust
FROM rust:1.60.0-bullseye

RUN rustup toolchain install nightly
RUN rustup override set nightly

# Create util
WORKDIR /usr/src/api
RUN cargo new util 
WORKDIR /usr/src/api/util
COPY api/util/Cargo.toml Cargo.toml

# Create dba 
WORKDIR /usr/src/api
RUN cargo new --lib dba 
WORKDIR /usr/src/api/dba
COPY api/dba/Cargo.toml Cargo.toml
RUN mkdir /usr/src/api/dba/src/bin
RUN echo "fn main() {}" > /usr/src/api/dba/src/bin/fn_dba.rs

# Cache all dependencies
RUN cargo test --no-run


# Copy over lib code
WORKDIR /usr/src/lib/dba
COPY lib/dba/src src

# Copy over util code
WORKDIR /usr/src/api/util
COPY api/util/src src

# Copy over dba code
WORKDIR /usr/src/api/dba
COPY api/dba/src src

CMD cargo run