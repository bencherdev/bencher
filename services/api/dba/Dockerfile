# Build Stage
# https://hub.docker.com/_/rust
FROM rust:1.60.0-bullseye as builder

RUN rustup toolchain install nightly

WORKDIR /usr/src/lib/reports
COPY lib/reports/Cargo.toml Cargo.toml
COPY lib/reports/src src

WORKDIR /usr/src/api/util
COPY api/util/Cargo.toml Cargo.toml
COPY api/util/src src
COPY api/util/migrations migrations

WORKDIR /usr/src/api/dba
COPY api/dba/Cargo.toml Cargo.toml
COPY api/dba/src src

RUN cargo +nightly build --release

# Bundle Stage
# https://hub.docker.com/_/debian
FROM debian:bullseye-slim
COPY --from=builder /usr/src/api/dba/target/release/fn_dba .
ENV PORT 8080
USER 1000
CMD ["./fn_dba"]