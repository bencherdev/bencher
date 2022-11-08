use bencher_json::project::report::JsonAdapter;

// pub mod json;
// pub mod rust;

#[derive(Clone, Copy, Debug, Default)]
pub enum Adapter {
    #[default]
    Json,
    RustTest,
    RustBench,
    // RustCriterion,
}

impl From<Adapter> for JsonAdapter {
    fn from(adapter: Adapter) -> Self {
        match adapter {
            Adapter::Json => Self::Json,
            Adapter::RustTest => Self::RustTest,
            Adapter::RustBench => Self::RustBench,
        }
    }
}
