# Build Stage
# https://hub.docker.com/_/rust
FROM rust:1.60.0-bullseye as builder

RUN rustup target add x86_64-unknown-linux-musl
WORKDIR /usr/src/app
COPY . .
RUN cargo build --release

# Bundle Stage
FROM scratch
COPY --from=builder /usr/src/app/target/release/demo .
ENV PORT 8080
USER 1000
CMD ["./demo"]