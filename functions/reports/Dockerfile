# Build Stage
# https://hub.docker.com/_/rust
FROM rust:1.60.0-bullseye as builder

RUN rustup toolchain install nightly
WORKDIR /usr/src/app
COPY Cargo.toml Cargo.toml
COPY ./src ./src
RUN cargo +nightly build --release

# Bundle Stage
# https://hub.docker.com/_/debian
FROM debian:bullseye-slim
COPY --from=builder /usr/src/app/target/release/fn_reports .
ENV PORT 8080
USER 1000
CMD ["./fn_reports"]