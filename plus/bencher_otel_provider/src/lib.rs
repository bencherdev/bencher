use std::time::Duration;

use bencher_config::Config;
use bencher_json::{
    BENCHER_API_VERSION, Url,
    system::config::{JsonOtel, OtelProtocol},
};
use opentelemetry::KeyValue;
use opentelemetry_otlp::{MetricExporter, Protocol, WithExportConfig as _};
use opentelemetry_sdk::{
    Resource,
    metrics::{PeriodicReader, SdkMeterProvider},
};
use slog::Logger;

// todo(epompeii): Move from fly.io
// https://fly.io/docs/machines/runtime-environment/#environment-variables
const FLY_APP_NAME: &str = "FLY_APP_NAME";
const FLY_MACHINE_ID: &str = "FLY_MACHINE_ID";

const BENCHER_NAMESPACE: &str = "bencher";

const DEFAULT_EXPORT_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum OtelProviderError {
    #[error("Failed to initialize OpenTelemetry: {0}")]
    Build(opentelemetry_otlp::ExporterBuildError),
}

/// Initialize and run OpenTelemetry for the server.
// https://docs.rs/opentelemetry-otlp/0.31.0/opentelemetry_otlp/index.html#using-with-prometheus
pub fn run_open_telemetry(log: &Logger, config: &Config) -> Result<(), OtelProviderError> {
    let Some(otel_config) = config
        .plus
        .as_ref()
        .and_then(|plus| plus.cloud.as_ref())
        .and_then(|cloud| cloud.otel.as_ref())
    else {
        slog::info!(log, "OpenTelemetry not configured, skipping initialization");
        return Ok(());
    };
    let JsonOtel {
        endpoint,
        protocol,
        interval,
    } = otel_config;

    let resource = otel_resource(log);
    let reader = otel_reader(endpoint, protocol, *interval)?;
    let provider = SdkMeterProvider::builder()
        .with_resource(resource)
        .with_reader(reader)
        .build();

    opentelemetry::global::set_meter_provider(provider);

    Ok(())
}

fn otel_resource(log: &Logger) -> Resource {
    // https://opentelemetry.io/docs/specs/semconv/registry/attributes/service/#service-attributes
    let attributes = [
        std::env::var(FLY_MACHINE_ID)
            .inspect_err(|e| {
                slog::debug!(log, "Failed to get {FLY_MACHINE_ID} from environment: {e}");
            })
            .ok()
            .map(|id| ("service.instance.id".to_owned(), id)),
        Some(("service.namespace".to_owned(), BENCHER_NAMESPACE.to_owned())),
        std::env::var(FLY_APP_NAME)
            .inspect_err(|e| {
                slog::debug!(log, "Failed to get {FLY_APP_NAME} from environment: {e}");
            })
            .ok()
            .map(|name| ("service.name".to_owned(), name)),
        Some(("service.version".to_owned(), BENCHER_API_VERSION.to_owned())),
    ]
    .into_iter()
    .flatten()
    .map(|(k, v)| KeyValue::new(k, v));

    Resource::builder().with_attributes(attributes).build()
}

fn otel_reader(
    endpoint: &Url,
    protocol: &OtelProtocol,
    interval: Option<u64>,
) -> Result<PeriodicReader<MetricExporter>, OtelProviderError> {
    let protocol = map_protocol(protocol);

    let exporter = MetricExporter::builder()
        .with_http()
        .with_endpoint(endpoint.as_ref())
        .with_protocol(protocol)
        .build()
        .map_err(OtelProviderError::Build)?;

    Ok(PeriodicReader::builder(exporter)
        .with_interval(interval.map_or(DEFAULT_EXPORT_INTERVAL, Duration::from_secs))
        .build())
}

fn map_protocol(protocol: &OtelProtocol) -> Protocol {
    match protocol {
        OtelProtocol::Grpc => Protocol::Grpc,
        OtelProtocol::HttpBinary => Protocol::HttpBinary,
        OtelProtocol::HttpJson => Protocol::HttpJson,
    }
}
