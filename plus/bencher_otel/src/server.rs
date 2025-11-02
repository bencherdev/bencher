use opentelemetry_otlp::{MetricExporter, Protocol, WithExportConfig as _};
use opentelemetry_sdk::metrics::SdkMeterProvider;

#[derive(Debug, thiserror::Error)]
pub enum OtelServerError {
    #[error("Failed to initialize OpenTelemetry: {0}")]
    Build(opentelemetry_otlp::ExporterBuildError),
}

/// Initialize and run OpenTelemetry for the server.
// https://docs.rs/opentelemetry-otlp/0.31.0/opentelemetry_otlp/index.html#using-with-prometheus
pub fn run_open_telemetry(endpoint: &str) -> Result<(), OtelServerError> {
    // Initialize OTLP exporter using HTTP binary protocol
    let exporter = MetricExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint(endpoint)
        .build()
        .map_err(OtelServerError::Build)?;

    // Create a meter provider with the OTLP Metric exporter
    let meter_provider = SdkMeterProvider::builder()
        .with_periodic_exporter(exporter)
        .build();
    opentelemetry::global::set_meter_provider(meter_provider);

    Ok(())
}
