# https://hub.docker.com/_/rust
FROM rust:1.75.0-bookworm as builder

ARG LITESTREAM_VERSION=0.3.13
ARG LITESTREAM_ARCH
ARG LITESTREAM_BIN=litestream-v${LITESTREAM_VERSION}-linux-${LITESTREAM_ARCH}
ARG ARCH
ARG ZIG_VERSION
ARG ZIG_BIN=zig-linux-${ARCH}-${ZIG_VERSION}
ARG ZIG_BUILD_VERSION=0.18.3
ARG GLIBC_VERSION=2.17

RUN apt-get update \
    && apt-get install -y \
    # Database
    sqlite3 libsqlite3-dev \
    # Plot
    pkg-config libfreetype6-dev libfontconfig1-dev \
    # Stipe
    ca-certificates
ENV LD_LIBRARY_PATH=/usr/lib:/usr/local/lib:$LD_LIBRARY_PATH

WORKDIR /tmp/litestream
RUN wget https://github.com/benbjohnson/litestream/releases/download/v${LITESTREAM_VERSION}/${LITESTREAM_BIN}.tar.gz
RUN tar -xzf ${LITESTREAM_BIN}.tar.gz
RUN cp -r /tmp/litestream /usr/bin/litestream

WORKDIR /tmp/zig
RUN wget https://ziglang.org/builds/${ZIG_BIN}.tar.xz
RUN tar -xf ${ZIG_BIN}.tar.xz -C /usr/local
ENV PATH="/usr/local/${ZIG_BIN}:${PATH}"

RUN cargo install --version ${ZIG_BUILD_VERSION} --locked --force cargo-zigbuild

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

RUN cargo zigbuild --target ${ARCH}-unknown-linux-gnu.${GLIBC_VERSION}
# RUN cargo zigbuild --release --target ${TARGET}.${GLIBC_VERSION}

WORKDIR /usr/lib/bencher
RUN cp /usr/lib/${ARCH}-linux-gnu/libexpat.so.1 libexpat.so.1
RUN cp /usr/lib/${ARCH}-linux-gnu/libfontconfig.so.1 libfontconfig.so.1
RUN cp /usr/lib/${ARCH}-linux-gnu/libfreetype.so.6 libfreetype.so.6
RUN cp /usr/lib/${ARCH}-linux-gnu/libpng16.so.16 libpng16.so.16
RUN cp /usr/lib/${ARCH}-linux-gnu/libbrotlicommon.so.1 libbrotlicommon.so.1
RUN cp /usr/lib/${ARCH}-linux-gnu/libbrotlidec.so.1 libbrotlidec.so.1
RUN cp /usr/lib/${ARCH}-linux-gnu/libz.so.1 libz.so.1

WORKDIR /usr/bin/bencher
RUN cp /usr/src/target/${ARCH}-unknown-linux-gnu/debug/api api
# RUN cp /usr/src/target/${ARCH}-unknown-linux-gnu/release/api api
RUN mkdir -p /usr/bin/bencher/data

# https://github.com/GoogleContainerTools/distroless/blob/main/cc/README.md
FROM gcr.io/distroless/cc-debian12
COPY --from=builder /etc/fonts /etc/fonts
COPY --from=builder /usr/include/fontconfig /usr/include/fontconfig
COPY --from=builder /usr/lib/bencher /usr/lib
COPY --from=builder /usr/share/fonts /usr/share/fonts

COPY --from=builder /usr/bin/litestream/litestream /usr/bin/litestream
COPY --from=builder /usr/bin/bencher /usr/bin/bencher

ENV PORT 61016

CMD ["/usr/bin/bencher/api"]
