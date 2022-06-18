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
COPY api/util/diesel.toml diesel.toml

WORKDIR /usr/src/api/admin
COPY api/admin/Cargo.toml Cargo.toml
COPY api/admin/src src

RUN cargo +nightly build --release

# Bundle Stage
# https://hub.docker.com/_/debian
FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y libpq5
COPY --from=builder /usr/src/api/admin/target/release/fn_admin .
ENV PORT 8080
USER 1000
CMD ["./fn_admin"]