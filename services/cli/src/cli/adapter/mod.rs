use report::Metrics;

use crate::{
    cli::{
        benchmark::Output,
        clap::CliAdapter,
    },
    BencherError,
};

pub mod json;
pub mod rust;

#[derive(Clone, Copy, Debug)]
pub enum Adapter {
    Json,
    RustCargoBench,
}

impl From<CliAdapter> for Adapter {
    fn from(adapter: CliAdapter) -> Self {
        match adapter {
            CliAdapter::Json => Adapter::Json,
            CliAdapter::RustCargoBench => Adapter::RustCargoBench,
        }
    }
}

impl Into<report::Adapter> for Adapter {
    fn into(self) -> report::Adapter {
        match self {
            Self::Json => report::Adapter::Json,
            Self::RustCargoBench => report::Adapter::RustCargoBench,
        }
    }
}

pub fn map_adapter(adapter: Option<CliAdapter>) -> Result<Adapter, BencherError> {
    if let Some(adapter) = adapter {
        Ok(adapter.into())
    } else {
        Ok(Adapter::Json)
    }
}

impl Adapter {
    pub fn convert(&self, output: &Output) -> Result<Metrics, BencherError> {
        match &self {
            Adapter::Json => json::parse(output),
            Adapter::RustCargoBench => rust::parse(output),
        }
    }
}
