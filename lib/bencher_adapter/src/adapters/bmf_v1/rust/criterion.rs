use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::rust::{
        criterion_json::AdapterRustCriterionJson, criterion_text::AdapterRustCriterionText,
    },
    results::adapter_results::AdapterResults,
};

/// criterion at version 1: cargo-criterion's JSON messages, then criterion's text.
pub struct AdapterRustCriterionV1;

impl Adaptable for AdapterRustCriterionV1 {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterRustCriterionJson::parse(input, settings)
            .or_else(|| AdapterRustCriterionText::parse(input, settings))
    }
}
