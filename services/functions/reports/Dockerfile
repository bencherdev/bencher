# Build Stage
# https://hub.docker.com/_/rust
FROM rust:1.60.0-bullseye as builder

RUN rustup toolchain install nightly

WORKDIR /usr/src/lib/reports
COPY lib/reports/Cargo.toml Cargo.toml
COPY lib/reports/src src

WORKDIR /usr/src/functions/util
COPY functions/util/Cargo.toml Cargo.toml
COPY functions/util/src src

WORKDIR /usr/src/functions/reports
COPY functions/reports/Cargo.toml Cargo.toml
COPY functions/reports/src src

RUN cargo +nightly build --release

# Bundle Stage
# https://hub.docker.com/_/debian
FROM debian:bullseye-slim
COPY --from=builder /usr/src/functions/reports/target/release/fn_reports .
ENV PORT 8080
USER 1000
CMD ["./fn_reports"]