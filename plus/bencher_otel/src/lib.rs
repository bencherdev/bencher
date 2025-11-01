use opentelemetry_otlp::{MetricExporter, Protocol, WithExportConfig as _};

#[derive(Debug, thiserror::Error)]
pub enum OpenTelemetryError {
    #[error("Failed to initialize OpenTelemetry: {0}")]
    Build(opentelemetry_otlp::ExporterBuildError),
}

fn run_open_telemetry() -> Result<opentelemetry_sdk::metrics::SdkMeterProvider, OpenTelemetryError>
{
    // Initialize OTLP exporter using HTTP binary protocol
    let exporter = MetricExporter::builder()
        .with_http()
        .with_protocol(Protocol::HttpBinary)
        .with_endpoint("http://0.0.0.0:9090/v0/server/metrics")
        .build()
        .map_err(OpenTelemetryError::Build)?;

    // Create a meter provider with the OTLP Metric exporter
    let meter_provider = opentelemetry_sdk::metrics::SdkMeterProvider::builder()
        .with_periodic_exporter(exporter)
        .build();
    opentelemetry::global::set_meter_provider(meter_provider.clone());

    Ok(meter_provider)
}
