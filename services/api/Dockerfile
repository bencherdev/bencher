# Build Stage
# https://hub.docker.com/_/rust
FROM rust:1.60.0-bullseye as builder

RUN rustup toolchain install nightly

WORKDIR /usr/src/lib
COPY lib/report report

WORKDIR /usr/src/api
COPY api/ .

RUN cargo +nightly build --release

# Bundle Stage
# https://hub.docker.com/r/litestream/litestream
FROM litestream/litestream:sha-565f7a4
COPY --from=builder /usr/src/api/target/release/api /api
COPY --from=builder /usr/src/api/litestream.yml /etc/litestream.yml
ENV PORT 8080
USER 1000
