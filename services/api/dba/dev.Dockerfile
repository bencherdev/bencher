# https://hub.docker.com/_/rust
FROM rust:1.60.0-bullseye

RUN apt-get update \
    && apt-get install -y netcat

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

# Create dba
WORKDIR /usr/src/api
RUN cargo new --lib dba 
WORKDIR /usr/src/api/dba
COPY api/dba/Cargo.toml Cargo.toml
RUN mkdir /usr/src/api/dba/src/bin
RUN echo "fn main() {}" > /usr/src/api/dba/src/bin/fn_dba.rs

# Cache all dependencies
RUN cargo test --no-run

# Add entrypoint.sh
COPY api/reports/entrypoint.sh /usr/src/api/dba/entrypoint.sh
RUN chmod +x /usr/src/api/dba/entrypoint.sh

# Copy over lib code
WORKDIR /usr/src/lib/reports
COPY lib/reports/src src

# Copy over util code
WORKDIR /usr/src/api/util
COPY api/util/src src
COPY api/util/migrations migrations

# Copy over dba code
WORKDIR /usr/src/api/dba
COPY api/dba/src src

CMD ["/usr/src/api/dba/entrypoint.sh"]
