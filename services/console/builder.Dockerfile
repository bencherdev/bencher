# https://hub.docker.com/_/rust
FROM rust:1.71.0-bookworm as wasm-builder

RUN apt-get update && \
    apt-get install -y clang

ARG MOLD_VERSION
RUN curl -L --retry 10 --silent --show-error https://github.com/rui314/mold/releases/download/v${MOLD_VERSION}/mold-${MOLD_VERSION}-$(uname -m)-linux.tar.gz | tar -C /usr/local --strip-components=1 -xzf -
RUN "$(realpath /usr/bin/ld)" != /usr/local/bin/mold && sudo ln -sf /usr/local/bin/mold "$(realpath /usr/bin/ld)"; true

RUN curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

WORKDIR /usr/src
COPY Cargo.toml Cargo.toml
RUN cargo init xtask

WORKDIR /usr/src/.cargo
COPY .cargo/config.toml config.toml

WORKDIR /usr/src/services
RUN cargo init api
RUN cargo init cli

WORKDIR /usr/src/plus
RUN cargo init bencher_plus --lib

WORKDIR /usr/src/lib
COPY lib/bencher_valid bencher_valid

WORKDIR /usr/src/lib/bencher_valid
RUN wasm-pack build --target web --features plus,wasm

# https://hub.docker.com/_/node
FROM node:lts-bookworm
COPY --from=wasm-builder /usr/src/lib/bencher_valid /usr/src/lib/bencher_valid

WORKDIR /usr/src/services/ui
COPY services/console/package-lock.json package-lock.json
COPY services/console/package.json package.json

RUN npm install

COPY services/console/public public
COPY services/console/src src
COPY services/console/astro.config.mjs astro.config.mjs
COPY services/console/site.js site.js
COPY services/console/tsconfig.json tsconfig.json
# COPY services/console/.env.runtime .env.runtime

# https://docs.astro.build/en/guides/integrations-guide/node
RUN npx astro add node -y

RUN npm run build
