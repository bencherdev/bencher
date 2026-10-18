use crate::{
    Adaptable, Settings,
    adapters::{json::v1::AdapterJsonV1, magic::AdapterMagic},
    results::adapter_results::AdapterResults,
};

/// The version 1 `magic`: each version 1 leaf in turn, then the version 0 `magic`.
pub struct AdapterMagicV1;

impl Adaptable for AdapterMagicV1 {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        AdapterJsonV1::parse(input, settings).or_else(|| AdapterMagic::parse(input, settings))
    }
}
