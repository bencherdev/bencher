pub mod alert;
pub mod archive;
pub mod benchmark;
pub mod branch;
pub mod job;
pub mod key;
pub mod measure;
pub mod metric;
pub mod perf;
pub mod plot;
#[expect(
    clippy::module_inception,
    reason = "module re-exports the primary type"
)]
pub mod project;
pub mod report;
pub mod testbed;
pub mod threshold;

/// The client spells a parameters filter as plain JSON, so the sets travel through it.
fn parameter_filter(
    sets: Vec<bencher_json::ParameterSet>,
) -> Result<bencher_client::types::ParameterFilter, serde_json::Error> {
    serde_json::to_value(bencher_json::ParameterFilter::new(sets)).and_then(serde_json::from_value)
}
