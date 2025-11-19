use opentelemetry::metrics::Meter;

pub struct ApiMeter {
    meter: Meter,
}

impl ApiMeter {
    const NAME: &str = "bencher_api";

    fn new() -> Self {
        let meter = opentelemetry::global::meter(Self::NAME);
        ApiMeter { meter }
    }

    pub fn increment(counter: ApiCounter) {
        let counter = Self::new()
            .meter
            .u64_counter(counter.name().to_owned())
            .with_description(counter.description().to_owned())
            .build();
        counter.add(1, &[]);
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ApiCounter {
    ServerStartup,
}

impl ApiCounter {
    fn name(&self) -> &str {
        match self {
            ApiCounter::ServerStartup => "server_startup",
        }
    }

    fn description(&self) -> &str {
        match self {
            ApiCounter::ServerStartup => "Counts the number of server startups",
        }
    }
}
