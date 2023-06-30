include!(concat!(env!("OUT_DIR"), "/codegen.rs"));

impl From<bencher_json::ResourceId> for types::ResourceId {
    fn from(resource_id: bencher_json::ResourceId) -> Self {
        Self(resource_id.to_string())
    }
}
