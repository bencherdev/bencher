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

# Create admin
WORKDIR /usr/src/api
RUN cargo new --lib admin 
WORKDIR /usr/src/api/admin
COPY api/admin/Cargo.toml Cargo.toml
RUN mkdir /usr/src/api/admin/src/bin
RUN echo "fn main() {}" > /usr/src/api/admin/src/bin/fn_admin.rs

# Cache all dependencies
RUN cargo test --no-run

# Add entrypoint.sh
COPY api/reports/entrypoint.sh /usr/src/api/admin/entrypoint.sh
RUN chmod +x /usr/src/api/admin/entrypoint.sh

# Copy over lib code
WORKDIR /usr/src/lib/reports
COPY lib/reports/src src

# Copy over util code
WORKDIR /usr/src/api/util
COPY api/util/src src
COPY api/util/migrations migrations
COPY api/util/diesel.toml diesel.toml

# Copy over admin code
WORKDIR /usr/src/api/admin
COPY api/admin/src src

CMD ["/usr/src/api/admin/entrypoint.sh"]
