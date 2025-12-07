FROM debian:bookworm-slim@sha256:b4aa902587c2e61ce789849cb54c332b0400fe27b1ee33af4669e1f7e7c3e22f AS download
# https://github.com/open-telemetry/opentelemetry-collector-releases/releases
ARG OTEL_COLLECTOR_VERSION=v0.139.0
# https://opentelemetry.io/docs/collector/installation/#deb-installation
RUN apt-get update && apt-get install -y curl && \
    curl -fsSL -o /tmp/otel-collector.tgz https://github.com/open-telemetry/opentelemetry-collector-releases/releases/download/${OTEL_COLLECTOR_VERSION}/otelcol-contrib_${OTEL_COLLECTOR_VERSION#v}_linux_amd64.tar.gz && \
    tar -C /tmp -xzf /tmp/otel-collector.tgz

FROM gcr.io/distroless/static-debian12@sha256:4b2a093ef4649bccd586625090a3c668b254cfe180dee54f4c94f3e9bd7e381e
COPY --from=download /tmp/otelcol-contrib /otelcol-contrib
COPY otel-collector.yaml /etc/otel-collector.yaml
EXPOSE 4317 4318 9464
USER 65532:65532
ENTRYPOINT ["/otelcol-contrib", "--config=/etc/otel-collector.yaml"]
