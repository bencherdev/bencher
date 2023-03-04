# Build Stage
# builder.Dockerfile
FROM bencher-api-builder as builder

# Bundle Stage
# https://hub.docker.com/_/debian
FROM debian:bullseye-slim
COPY --from=builder /usr/src/target/release/api /api
RUN mkdir -p /data

RUN apt-get update \
    && apt-get install -y sqlite3 chromium wget sudo systemctl

RUN wget https://github.com/benbjohnson/litestream/releases/download/v0.3.9/litestream-v0.3.9-linux-amd64.deb
RUN dpkg -i litestream-v0.3.9-linux-amd64.deb

COPY entrypoint.sh /entrypoint.sh
ENV PORT 61016

CMD ["/entrypoint.sh"]
