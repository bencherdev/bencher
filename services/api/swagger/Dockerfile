# Build Stage
# https://hub.docker.com/_/rust
FROM rust:1.60.0-bullseye as builder

RUN rustup toolchain install nightly

WORKDIR /usr/src/lib/reports
COPY lib/reports/Cargo.toml Cargo.toml
COPY lib/reports/src src

WORKDIR /usr/src/api/util
COPY api/util/Cargo.toml Cargo.toml
COPY api/util/src src
COPY api/util/migrations migrations
COPY api/util/diesel.toml diesel.toml

WORKDIR /usr/src/api/dba
COPY api/dba/Cargo.toml Cargo.toml
COPY api/dba/src src

WORKDIR /usr/src/api/reports
COPY api/reports/Cargo.toml Cargo.toml
COPY api/reports/src src

WORKDIR /usr/src/api/swagger
COPY api/swagger/Cargo.toml Cargo.toml
COPY api/swagger/src src

RUN cargo +nightly run > /usr/src/api/swagger/swagger.json

# https://hub.docker.com/r/swaggerapi/swagger-ui
FROM swaggerapi/swagger-ui:v4.11.1

COPY --from=builder /usr/src/api/swagger/swagger.json /swagger.json
ENV SWAGGER_JSON /swagger.json
ENV BASE_URL /v0