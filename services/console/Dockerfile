# https://hub.docker.com/_/rust
FROM rust:1.79.0-bookworm as wasm-builder

RUN curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

WORKDIR /usr/src/bencher/lib
RUN cargo init --lib bencher_adapter
RUN cargo init --lib bencher_boundary
RUN cargo init --lib bencher_comment
RUN cargo init --lib bencher_github
RUN cargo init --lib bencher_json
RUN cargo init --lib bencher_logger
RUN cargo init --lib bencher_plot
RUN cargo init --lib bencher_rank
RUN cargo init --lib bencher_rbac
RUN cargo init --lib bencher_token
COPY lib/bencher_valid bencher_valid

WORKDIR /usr/src/bencher/plus
RUN cargo init --lib bencher_billing
RUN cargo init --lib bencher_bing_index
RUN cargo init --lib bencher_license
RUN cargo init --lib bencher_google_index

WORKDIR /usr/src/bencher/tasks
RUN cargo init --bin bin_version
RUN cargo init --bin gen_installer
RUN cargo init --bin gen_pkg
RUN cargo init --bin gen_notes
RUN cargo init --bin get_types
RUN cargo init --bin test_api
RUN cargo init --bin test_netlify

WORKDIR /usr/src/bencher
COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
RUN cargo init xtask

WORKDIR /usr/src/bencher/services
RUN cargo init api
RUN cargo init cli

WORKDIR /usr/src/bencher/services/console
COPY services/console/build_wasm.sh build_wasm.sh
RUN chmod +x build_wasm.sh
RUN ./build_wasm.sh

# https://hub.docker.com/_/node
FROM node:20.11.0-bookworm as builder
COPY services/api/openapi.json /usr/src/bencher/services/api/openapi.json
COPY services/cli/templates/output /usr/src/bencher/services/cli/templates/output
COPY --from=wasm-builder /usr/src/bencher/lib/bencher_valid/pkg /usr/src/bencher/lib/bencher_valid/pkg

WORKDIR /usr/src/bencher/services/console
COPY services/console/package-lock.json package-lock.json
COPY services/console/package.json package.json

RUN npm ci

COPY services/console/public public
COPY services/console/src src
COPY services/console/astro.config.mjs astro.config.mjs
COPY services/console/adapter.js adapter.js
COPY services/console/site.js site.js
COPY services/console/tsconfig.json tsconfig.json
COPY services/console/.env.runtime .env.runtime

RUN npm run node

# https://hub.docker.com/_/node
FROM node:20.11.0-bookworm as packager
COPY --from=builder /usr/src/bencher/services/console/dist /usr/src/bencher/services/console/dist
COPY --from=builder /usr/src/bencher/services/console/package-lock.json /usr/src/bencher/services/console/package-lock.json
COPY --from=builder /usr/src/bencher/services/console/package.json /usr/src/bencher/services/console/package.json

WORKDIR /usr/src/bencher/services/console
# https://github.com/withastro/astro/issues/7247#issuecomment-1576200139
# https://github.com/GoogleContainerTools/distroless/blob/main/examples/nodejs/Dockerfile
RUN npm ci --omit=dev

# https://github.com/GoogleContainerTools/distroless/tree/main/nodejs
FROM gcr.io/distroless/nodejs20-debian12
COPY --from=packager /usr/src/bencher/services/console /usr/src/bencher

ENV HOST=0.0.0.0
ENV PORT=3000
ENV BENCHER_API_URL=http://localhost:61016
EXPOSE 3000

CMD ["/usr/src/bencher/dist/server/entry.mjs"]