# https://hub.docker.com/_/rust
FROM rust:1.91.1-bookworm@sha256:8fed34f697cc63b2c9bb92233b4c078667786834d94dd51880cd0184285eefcf AS builder

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
COPY lib/api_auth api_auth
COPY lib/api_checkout api_checkout
COPY lib/api_organizations api_organizations
COPY lib/api_projects api_projects
COPY lib/api_run api_run
COPY lib/api_server api_server
COPY lib/api_users api_users
COPY lib/bencher_adapter bencher_adapter
COPY lib/bencher_boundary bencher_boundary
COPY lib/bencher_client bencher_client
COPY lib/bencher_comment bencher_comment
COPY lib/bencher_config bencher_config
COPY lib/bencher_context bencher_context
COPY lib/bencher_endpoint bencher_endpoint
COPY lib/bencher_json bencher_json
COPY lib/bencher_logger bencher_logger
COPY lib/bencher_parser bencher_parser
COPY lib/bencher_plot bencher_plot
COPY lib/bencher_rank bencher_rank
COPY lib/bencher_rbac bencher_rbac
COPY lib/bencher_schema bencher_schema
COPY lib/bencher_token bencher_token
COPY lib/bencher_valid bencher_valid
RUN cargo init --lib bencher_api_tests

WORKDIR /usr/src/bencher/plus
COPY plus/api_runners api_runners
COPY plus/api_specs api_specs
COPY plus/api_oci api_oci
COPY plus/bencher_billing bencher_billing
COPY plus/bencher_bing_index bencher_bing_index
COPY plus/bencher_github_client bencher_github_client
COPY plus/bencher_google_client bencher_google_client
COPY plus/bencher_google_index bencher_google_index
COPY plus/bencher_license bencher_license
COPY plus/bencher_oci bencher_oci
COPY plus/bencher_oci_storage bencher_oci_storage
COPY plus/bencher_output_protocol bencher_output_protocol
COPY plus/bencher_otel bencher_otel
COPY plus/bencher_otel_provider bencher_otel_provider
COPY plus/bencher_recaptcha bencher_recaptcha
COPY plus/bencher_rootfs bencher_rootfs
COPY plus/bencher_runner bencher_runner
COPY plus/bencher_init bencher_init

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
FROM gcr.io/distroless/cc-debian12@sha256:0c8eac8ea42a167255d03c3ba6dfad2989c15427ed93d16c53ef9706ea4691df

COPY --from=builder /usr/src/bencher/target/release/bencher /usr/bin/bencher

ENTRYPOINT ["/usr/bin/bencher"]
