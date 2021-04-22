# https://hub.docker.com/_/rust
FROM rust:1.51.0

RUN apt-get update && \
    apt-get install netcat-openbsd

WORKDIR /usr/src/
RUN USER=root cargo new exec --lib
WORKDIR /usr/src/exec
COPY Cargo.toml .
COPY Cargo.lock .
RUN cargo test --no-run

COPY entrypoint.sh .
RUN ["chmod", "+x", "/usr/src/exec/entrypoint.sh"]
COPY src ./src

ENV ENDPOINT_ADDRESS $ENDPOINT_ADDRESS
ENV ENDPOINT_PORT $ENDPOINT_PORT
ENV DATA_PATH $DATA_PATH

CMD ["sh", "/usr/src/exec/entrypoint.sh"]