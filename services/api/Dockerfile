# Build Stage
# https://hub.docker.com/_/rust
FROM rust:1.60.0-bullseye as builder

RUN rustup toolchain install nightly

WORKDIR /usr/src/lib/report
COPY lib/report/Cargo.toml Cargo.toml
COPY lib/report/src src

WORKDIR /usr/src/api
COPY api/Cargo.toml Cargo.toml
COPY api/src src

RUN cargo +nightly build --release

# Bundle Stage
# https://hub.docker.com/r/litestream/litestream
FROM litestream/litestream:sha-565f7a4
COPY --from=builder /usr/src/api/target/release/api .
ENV PORT 8080
USER 1000
CMD ["./api"]