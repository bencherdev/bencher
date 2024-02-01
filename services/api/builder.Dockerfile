# https://hub.docker.com/_/rust
FROM rust:1.75.0-bookworm

RUN apt-get update \
    && apt-get install -y \
    # Database
    sqlite3 libsqlite3-dev \
    # Plot
    pkg-config libfreetype6-dev libfontconfig1-dev \
    # Stipe
    ca-certificates
WORKDIR /usr/src/target/debug/deps
# WORKDIR /usr/src/target/release/deps
RUN ln -s /usr/bin/sqlite3 /usr/src/target/debug/deps/libsqlite3.so
# RUN ln -s /usr/bin/sqlite3 /usr/src/target/release/deps/libsqlite3.so
ENV LD_LIBRARY_PATH=/usr/lib:/usr/local/lib:$LD_LIBRARY_PATH

WORKDIR /tmp
ARG ZIG_BIN
RUN wget https://ziglang.org/builds/${ZIG_BIN}.tar.xz
RUN tar -xf ${ZIG_BIN}.tar.xz -C /usr/local
ENV PATH="/usr/local/${ZIG_BIN}:${PATH}"

ARG ZIG_VERSION
RUN cargo install --version ${ZIG_VERSION} --locked --force cargo-zigbuild

WORKDIR /data

WORKDIR /usr/src/lib
COPY lib/bencher_adapter bencher_adapter
COPY lib/bencher_boundary bencher_boundary
COPY lib/bencher_comment bencher_comment
COPY lib/bencher_github bencher_github
COPY lib/bencher_json bencher_json
COPY lib/bencher_logger bencher_logger
COPY lib/bencher_plot bencher_plot
COPY lib/bencher_rbac bencher_rbac
COPY lib/bencher_token bencher_token
COPY lib/bencher_valid bencher_valid

WORKDIR /usr/src/plus
COPY plus/bencher_billing bencher_billing
COPY plus/bencher_bing_index bencher_bing_index
COPY plus/bencher_license bencher_license
COPY plus/bencher_google_index bencher_google_index

WORKDIR /usr/src
COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
RUN cargo init xtask

WORKDIR /usr/src/services
RUN cargo init cli

WORKDIR /usr/src/services/api
COPY services/api/migrations migrations
COPY services/api/src src
COPY services/api/Cargo.toml Cargo.toml
COPY services/api/diesel.toml diesel.toml
COPY services/api/swagger.json swagger.json

ARG TARGET
ARG GLIBC_VERSION
RUN cargo zigbuild --target ${TARGET}.${GLIBC_VERSION}
# RUN cargo zigbuild --release --target ${TARGET}.${GLIBC_VERSION}
