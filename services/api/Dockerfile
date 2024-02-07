# https://hub.docker.com/_/rust
FROM rust:1.75.0-bookworm as builder

RUN apt-get update \
    && apt-get install -y \
    # Linker
    clang \
    # Database
    sqlite3 libsqlite3-dev \
    # Plot
    pkg-config libfreetype6-dev libfontconfig1-dev \
    # Stipe
    ca-certificates

WORKDIR /tmp/litestream
ARG LITESTREAM_VERSION
ARG LITESTREAM_ARCH
ARG LITESTREAM_BIN=litestream-v${LITESTREAM_VERSION}-linux-${LITESTREAM_ARCH}
RUN wget https://github.com/benbjohnson/litestream/releases/download/v${LITESTREAM_VERSION}/${LITESTREAM_BIN}.tar.gz
RUN tar -xzf ${LITESTREAM_BIN}.tar.gz

WORKDIR /tmp/mold
ARG MOLD_VERSION
RUN curl -L --retry 10 --silent --show-error https://github.com/rui314/mold/releases/download/v${MOLD_VERSION}/mold-${MOLD_VERSION}-$(uname -m)-linux.tar.gz | tar -C /usr/local --strip-components=1 -xzf -
RUN "$(realpath /usr/bin/ld)" != /usr/local/bin/mold && sudo ln -sf /usr/local/bin/mold "$(realpath /usr/bin/ld)"; true

WORKDIR /usr/src/bencher/.cargo
COPY .cargo/config.toml config.toml

WORKDIR /usr/src/bencher/lib
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

WORKDIR /usr/src/bencher/plus
COPY plus/bencher_billing bencher_billing
COPY plus/bencher_bing_index bencher_bing_index
COPY plus/bencher_license bencher_license
COPY plus/bencher_google_index bencher_google_index

WORKDIR /usr/src/bencher
COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
RUN cargo init xtask

WORKDIR /usr/src/bencher/services
RUN cargo init cli

WORKDIR /usr/src/bencher/services/api
COPY services/api/migrations migrations
COPY services/api/src src
COPY services/api/Cargo.toml Cargo.toml
COPY services/api/diesel.toml diesel.toml
COPY services/api/swagger.json swagger.json

RUN cargo build --release

WORKDIR /usr/local/bencher-deps
RUN cp /usr/lib/$(uname -m)-linux-gnu/libexpat.so.1 libexpat.so.1
RUN cp /usr/lib/$(uname -m)-linux-gnu/libfontconfig.so.1 libfontconfig.so.1
RUN cp /usr/lib/$(uname -m)-linux-gnu/libfreetype.so.6 libfreetype.so.6
RUN cp /usr/lib/$(uname -m)-linux-gnu/libpng16.so.16 libpng16.so.16
RUN cp /usr/lib/$(uname -m)-linux-gnu/libbrotlicommon.so.1 libbrotlicommon.so.1
RUN cp /usr/lib/$(uname -m)-linux-gnu/libbrotlidec.so.1 libbrotlidec.so.1
RUN cp /usr/lib/$(uname -m)-linux-gnu/libz.so.1 libz.so.1

WORKDIR /usr/lib/bencher
RUN cp /usr/src/bencher/target/release/api api
WORKDIR /var/lib/bencher/data

# https://github.com/GoogleContainerTools/distroless/blob/main/cc/README.md
FROM gcr.io/distroless/cc-debian12
COPY --from=builder /tmp/litestream/litestream /usr/bin/litestream

COPY --from=builder /etc/fonts /etc/fonts
COPY --from=builder /usr/include/fontconfig /usr/include/fontconfig
COPY --from=builder /usr/local/bencher-deps /usr/lib
COPY --from=builder /usr/share/fonts /usr/share/fonts

# Executable
COPY --from=builder /usr/lib/bencher /usr/lib/bencher
# Configuration
VOLUME /etc/bencher
# Database
COPY --from=builder /var/lib/bencher/data /var/lib/bencher/data
VOLUME /var/lib/bencher
# Logs
VOLUME /var/log/bencher

EXPOSE 61016

CMD ["/usr/lib/bencher/api"]
