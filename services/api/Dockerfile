# Build Stage
# https://hub.docker.com/_/rust
FROM rust:1.63.0-bullseye as builder

WORKDIR /usr/src/lib
COPY lib/bencher_json bencher_json
COPY lib/bencher_rbac bencher_rbac

WORKDIR /usr/src/api
COPY api/src src
COPY api/Cargo.toml Cargo.toml
COPY api/migrations migrations
COPY api/diesel.toml diesel.toml

RUN cargo build --release

COPY api/.env .env

# Bundle Stage
# https://hub.docker.com/_/debian
FROM debian:bullseye-slim
COPY --from=builder /usr/src/api/target/release/api /api
COPY --from=builder /usr/src/api/.env /.env

RUN apt-get update \
    && apt-get install -y sqlite3

ENV PORT 8080
# USER 1000

CMD ["/api"]
