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
    sqlite3 \
    pkg-config libfreetype6-dev libfontconfig1-dev

ENV PORT 61016

CMD ["/api"]
