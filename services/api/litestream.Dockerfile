# Build Stage
# builder.Dockerfile
FROM bencher-api-builder as builder

# Bundle Stage
# https://hub.docker.com/_/debian
FROM debian:bookworm-slim
COPY --from=builder /usr/src/target/release/api /api
RUN mkdir -p /data

RUN apt-get update \
    && apt-get install -y \
    # Database
    sqlite3 \
    # Plot
    pkg-config libfreetype6-dev libfontconfig1-dev \
    # Stipe
    ca-certificates \
    # Litestream
    wget sudo systemctl

ARG LITESTREAM_VERSION
RUN echo "litestream v$LITESTREAM_VERSION"
RUN wget https://github.com/benbjohnson/litestream/releases/download/v${LITESTREAM_VERSION}/litestream-v${LITESTREAM_VERSION}-linux-amd64.deb
RUN dpkg -i litestream-v${LITESTREAM_VERSION}-linux-amd64.deb

COPY entrypoint.sh /entrypoint.sh
ENV PORT 61016

CMD ["/entrypoint.sh"]
