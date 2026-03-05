# https://hub.docker.com/_/rust
FROM rust:1.91.1-bookworm AS builder

RUN apt-get update \
    && apt-get install -y \
    # Linker
    clang \
    # TLS certificates
    ca-certificates

WORKDIR /tmp/mold
ARG MOLD_VERSION
RUN curl -L --retry 10 --silent --show-error https://github.com/rui314/mold/releases/download/v${MOLD_VERSION}/mold-${MOLD_VERSION}-$(uname -m)-linux.tar.gz | tar -C /usr/local --strip-components=1 -xzf -
RUN "$(realpath /usr/bin/ld)" != /usr/local/bin/mold && sudo ln -sf /usr/local/bin/mold "$(realpath /usr/bin/ld)"; true

WORKDIR /usr/src/bencher/.cargo
COPY .cargo/config.toml config.toml

WORKDIR /usr/src/bencher/lib
RUN cargo init --lib api_auth
RUN cargo init --lib api_checkout
RUN cargo init --lib api_organizations
RUN cargo init --lib api_projects
RUN cargo init --lib api_run
RUN cargo init --lib api_server
RUN cargo init --lib api_users
RUN cargo init --lib bencher_api_tests
RUN cargo init --lib bencher_boundary
RUN cargo init --lib bencher_config
RUN cargo init --lib bencher_endpoint
RUN cargo init --lib bencher_logger
RUN cargo init --lib bencher_plot
RUN cargo init --lib bencher_rank
RUN cargo init --lib bencher_rbac
RUN cargo init --lib bencher_schema
RUN cargo init --lib bencher_token
COPY lib/bencher_adapter bencher_adapter
COPY lib/bencher_client bencher_client
COPY lib/bencher_comment bencher_comment
COPY lib/bencher_context bencher_context
COPY lib/bencher_json bencher_json
COPY lib/bencher_parser bencher_parser
COPY lib/bencher_valid bencher_valid

WORKDIR /usr/src/bencher/plus
RUN cargo init --lib api_oci
RUN cargo init --lib api_runners
RUN cargo init --lib api_specs
RUN cargo init --lib bencher_billing
RUN cargo init --lib bencher_bing_index
RUN cargo init --lib bencher_github_client
RUN cargo init --lib bencher_google_client
RUN cargo init --lib bencher_google_index
RUN cargo init --lib bencher_license
RUN cargo init --lib bencher_oci
RUN cargo init --lib bencher_oci_storage
RUN cargo init --lib bencher_output_protocol
RUN cargo init --lib bencher_otel
RUN cargo init --lib bencher_otel_provider
RUN cargo init --lib bencher_recaptcha
RUN cargo init --lib bencher_rootfs
RUN cargo init --lib bencher_runner
RUN cargo init --lib bencher_init

WORKDIR /usr/src/bencher/tasks
RUN cargo init --bin bin_version
RUN cargo init --bin gen_installer
RUN cargo init --bin gen_pkg
RUN cargo init --bin gen_notes
RUN cargo init --bin gen_types
RUN cargo init --bin test_api
RUN cargo init --bin test_netlify
RUN cargo init --bin test_runner

WORKDIR /usr/src/bencher
COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
RUN cargo init xtask

WORKDIR /usr/src/bencher/services
RUN cargo init api
RUN cargo init runner

# The bencher_client build.rs reads services/api/openapi.json
COPY services/api/openapi.json /usr/src/bencher/services/api/openapi.json

WORKDIR /usr/src/bencher/services/cli
COPY services/cli/src src
COPY services/cli/Cargo.toml Cargo.toml

RUN cargo build --release -p bencher_cli

# https://github.com/GoogleContainerTools/distroless/blob/main/cc/README.md
FROM gcr.io/distroless/cc-debian12

COPY --from=builder /usr/src/bencher/target/release/bencher /usr/bin/bencher

ENTRYPOINT ["/usr/bin/bencher"]
