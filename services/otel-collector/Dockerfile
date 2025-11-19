FROM debian:bookworm-20251117-slim AS download
# https://github.com/open-telemetry/opentelemetry-collector-releases/releases
ARG OTEL_COLLECTOR_VERSION=v0.139.0
# https://opentelemetry.io/docs/collector/installation/#deb-installation
RUN apt-get update && apt-get install -y curl && \
    curl -fsSL -o /tmp/otel-collector.tgz https://github.com/open-telemetry/opentelemetry-collector-releases/releases/download/${OTEL_COLLECTOR_VERSION}/otelcol-contrib_${OTEL_COLLECTOR_VERSION#v}_linux_amd64.tar.gz && \
    tar -C /tmp -xzf /tmp/otel-collector.tgz

FROM gcr.io/distroless/static-debian12
COPY --from=download /tmp/otelcol-contrib /otelcol-contrib
COPY otel-collector.yaml /etc/otel-collector.yaml
EXPOSE 4317 4318 9464
USER 65532:65532
ENTRYPOINT ["/otelcol-contrib", "--config=/etc/otel-collector.yaml"]
