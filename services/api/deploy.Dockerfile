# Build Stage
# build.Dockerfile
FROM build-bencher-api as builder

# Bundle Stage
# https://hub.docker.com/_/debian
FROM debian:bullseye-slim
COPY --from=builder /usr/src/api/target/release/api /api

RUN apt-get update \
    && apt-get install -y wget sudo systemctl sqlite3

RUN wget https://github.com/benbjohnson/litestream/releases/download/v0.3.9/litestream-v0.3.9-linux-amd64.deb
RUN dpkg -i litestream-v0.3.9-linux-amd64.deb
COPY api/litestream.yml /etc/litestream.yml

COPY api/entrypoint.sh /entrypoint.sh
ENV PORT 61016
# USER 1000

CMD ["/entrypoint.sh"]
