# https://hub.docker.com/_/rust
FROM rust:1.60.0-bullseye

RUN apt-get update \
    && apt-get install -y sqlite3

RUN rustup toolchain install nightly
RUN rustup override set nightly

# Init lib/report
WORKDIR /usr/src/lib
RUN cargo new report
WORKDIR /usr/src/lib/report
COPY lib/report/Cargo.toml Cargo.toml

# Init api
WORKDIR /usr/src/api
RUN cargo new --lib api
COPY api/Cargo.toml Cargo.toml
RUN mkdir /usr/src/api/src/bin
RUN echo "fn main() {}" > /usr/src/api/src/bin/api.rs

# Cache all dependencies
RUN cargo test --no-run

# Copy over lib/report code
WORKDIR /usr/src/lib/report
COPY lib/report/src src

# Copy over api code
WORKDIR /usr/src/api
COPY api/src src
COPY api/migrations migrations
COPY api/diesel.toml diesel.toml

CMD ["cargo", "run"]
