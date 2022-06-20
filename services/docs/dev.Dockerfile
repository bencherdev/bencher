# https://hub.docker.com/_/rust
FROM rust:1.60.0-bullseye

RUN cargo install mdbook --vers "^0.4" --locked
WORKDIR /usr/src/docs

CMD ["mdbook", "serve", "-n", "0.0.0.0"]