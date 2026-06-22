# syntax=docker/dockerfile:1
# https://hub.docker.com/_/rust
FROM rust:1.95.0-bookworm@sha256:503651ea31e66ecb74623beabde781059a5978df1595a9e8ed03974d5fec1bf0 AS base

FROM base AS chef

RUN apt-get update \
    && apt-get install -y \
    # Linker
    clang \
    # Plot
    pkg-config libfreetype6-dev libfontconfig1-dev \
    # Stripe
    ca-certificates

WORKDIR /tmp/mold
ARG MOLD_VERSION
RUN curl -L --retry 10 --silent --show-error https://github.com/rui314/mold/releases/download/v${MOLD_VERSION}/mold-${MOLD_VERSION}-$(uname -m)-linux.tar.gz | tar -C /usr/local --strip-components=1 -xzf -
RUN "$(realpath /usr/bin/ld)" != /usr/local/bin/mold && sudo ln -sf /usr/local/bin/mold "$(realpath /usr/bin/ld)"; true

# cargo-chef caches the dependency build as its own layer; sccache caches each crate's
# compilation in S3-compatible object storage so only changed first-party crates are rebuilt.
ARG CARGO_CHEF_VERSION
ARG SCCACHE_VERSION
RUN cargo install cargo-chef --locked --version ${CARGO_CHEF_VERSION} \
    && cargo install sccache --locked --version ${SCCACHE_VERSION}

WORKDIR /usr/src/bencher

# Compute the dependency recipe from the workspace manifests + lockfile.
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Litestream is a runtime-only binary; keep it in its own stage so its version is decoupled
# from the build layers (a Litestream bump never invalidates the dependency cache).
FROM base AS litestream
WORKDIR /tmp/litestream
ARG LITESTREAM_VERSION
ARG TARGETARCH
RUN LITESTREAM_ARCH=$(case "${TARGETARCH}" in amd64) echo "x86_64" ;; arm64) echo "arm64" ;; *) echo "${TARGETARCH}" ;; esac) && \
    LITESTREAM_BIN="litestream-${LITESTREAM_VERSION}-linux-${LITESTREAM_ARCH}" && \
    wget "https://github.com/benbjohnson/litestream/releases/download/v${LITESTREAM_VERSION}/${LITESTREAM_BIN}.tar.gz" && \
    tar -xzf "${LITESTREAM_BIN}.tar.gz"

FROM chef AS builder
# "sccache" for trusted builds; empty for untrusted (fork) builds, which disables sccache
# entirely so the absent S3 credentials are never needed (a cold but correct build).
ARG RUSTC_WRAPPER=""
ENV RUSTC_WRAPPER=${RUSTC_WRAPPER}
ENV CARGO_INCREMENTAL=0
# sccache S3-compatible object storage backend.
ARG SCCACHE_BUCKET
ARG SCCACHE_ENDPOINT
ENV SCCACHE_BUCKET=${SCCACHE_BUCKET}
ENV SCCACHE_ENDPOINT=${SCCACHE_ENDPOINT}
ENV SCCACHE_REGION=auto

# `.cargo/config.toml` (mold linker + v0 symbol mangling) must be identical during `cook`
# and the final build, or cargo treats the cooked artifacts as stale and rebuilds them.
WORKDIR /usr/src/bencher/.cargo
COPY .cargo/config.toml config.toml
WORKDIR /usr/src/bencher

# Cook only the `api` binary's dependency graph. This layer is keyed on `recipe.json`
# (derived from `Cargo.lock` + the manifests), so it is reused whenever dependencies are
# unchanged. The S3 credential mounts are `required=false`: absent for untrusted builds,
# where `RUSTC_WRAPPER` is empty so sccache (and thus object storage) is never invoked.
COPY --from=planner /usr/src/bencher/recipe.json recipe.json
RUN --mount=type=secret,id=s3_access_key_id,required=false \
    --mount=type=secret,id=s3_secret_access_key,required=false \
    AWS_ACCESS_KEY_ID="$(cat /run/secrets/s3_access_key_id 2>/dev/null || true)" \
    AWS_SECRET_ACCESS_KEY="$(cat /run/secrets/s3_secret_access_key 2>/dev/null || true)" \
    cargo chef cook --release --bin api --recipe-path recipe.json \
    && (sccache --show-stats || true)

# Build the first-party code. Dependencies are already compiled above; sccache restores any
# unchanged first-party crates so only what actually changed is recompiled.
COPY . .
RUN --mount=type=secret,id=s3_access_key_id,required=false \
    --mount=type=secret,id=s3_secret_access_key,required=false \
    AWS_ACCESS_KEY_ID="$(cat /run/secrets/s3_access_key_id 2>/dev/null || true)" \
    AWS_SECRET_ACCESS_KEY="$(cat /run/secrets/s3_secret_access_key 2>/dev/null || true)" \
    cargo build --release --bin api \
    && (sccache --show-stats || true)

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
FROM gcr.io/distroless/cc-debian12@sha256:0c8eac8ea42a167255d03c3ba6dfad2989c15427ed93d16c53ef9706ea4691df
COPY --from=litestream /tmp/litestream/litestream /usr/bin/litestream

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

EXPOSE 6610

CMD ["/usr/lib/bencher/api"]
