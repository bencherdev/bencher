# https://hub.docker.com/_/rust
FROM rust:1.72.0-bookworm as wasm-builder

RUN curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

WORKDIR /usr/src
COPY Cargo.toml Cargo.toml
RUN cargo init xtask

WORKDIR /usr/src/services
RUN cargo init api
RUN cargo init cli

WORKDIR /usr/src/plus
RUN cargo init bencher_plus --lib

WORKDIR /usr/src/lib
COPY lib/bencher_valid bencher_valid

WORKDIR /usr/src/lib/bencher_valid
RUN wasm-pack build --target web --features plus,wasm

# https://hub.docker.com/_/node/
FROM node:lts-bookworm
COPY --from=wasm-builder /usr/src/lib/bencher_valid /usr/src/lib/bencher_valid

WORKDIR /usr/src/services/ui
COPY services/ui/package-lock.json package-lock.json
COPY services/ui/package.json package.json

RUN npm install

COPY services/ui/public public
COPY services/ui/src src
COPY services/ui/index.html index.html
COPY services/ui/tsconfig.json tsconfig.json
COPY services/ui/vite.config.ts vite.config.ts

RUN npm run build
