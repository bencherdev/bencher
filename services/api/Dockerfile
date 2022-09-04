# Build Stage
# https://hub.docker.com/_/rust
FROM rust:1.63.0-bullseye as builder

WORKDIR /usr/src/lib
COPY lib/bencher_json bencher_json

WORKDIR /usr/src/api
COPY api/src src
COPY api/Cargo.toml Cargo.toml
COPY api/migrations migrations
COPY api/diesel.toml diesel.toml

RUN cargo build --release

# Bundle Stage
# https://hub.docker.com/_/debian
FROM debian:bullseye-slim
COPY --from=builder /usr/src/api/target/release/api /api

RUN apt-get update \
    && apt-get install -y wget sudo systemctl

RUN wget https://github.com/benbjohnson/litestream/releases/download/v0.3.9/litestream-v0.3.9-linux-amd64.deb
RUN dpkg -i litestream-v0.3.9-linux-amd64.deb
COPY api/litestream.yml /etc/litestream.yml

COPY api/entrypoint.sh /entrypoint.sh
ENV PORT 8080
# USER 1000

CMD ["/entrypoint.sh"]
