# Build Stage
# build.Dockerfile
FROM build-bencher-api as builder

# Bundle Stage
# https://hub.docker.com/_/debian
FROM debian:bullseye-slim
COPY --from=builder /usr/src/target/release/api /api

RUN apt-get update \
    && apt-get install -y sqlite3

ENV PORT 61016
# USER 1000

CMD ["/api"]
