use std::convert::TryFrom;

use ::clap::Parser;
use report::{Report, Reports};

pub mod adapter;
pub mod backend;
pub mod benchmark;
pub mod clap;

use crate::cli::clap::CliBencher;
use crate::BencherError;
use adapter::Adapter;
use backend::Backend;
use benchmark::Benchmark;
use benchmark::Output;

#[derive(Debug)]
pub struct Bencher {
    benchmark: Benchmark,
    adapter: Adapter,
    backend: Option<Backend>,
}

impl TryFrom<CliBencher> for Bencher {
    type Error = BencherError;

    fn try_from(bencher: CliBencher) -> Result<Self, Self::Error> {
        Ok(Self {
            benchmark: Benchmark::try_from((bencher.shell, bencher.flag, bencher.cmd))?,
            adapter: Adapter::from(bencher.adapter),
            backend: if let Some(backend) = bencher.backend {
                Some(Backend::from(backend))
            } else {
                None
            },
        })
    }
}

impl Bencher {
    pub fn new() -> Result<Self, BencherError> {
        let args = CliBencher::parse();
        Self::try_from(args)
    }

    pub fn run(&self) -> Result<Output, BencherError> {
        self.benchmark.run()
    }

    pub fn convert(&self, output: Output) -> Result<Report, BencherError> {
        self.adapter.convert(output)
    }

    pub fn output(&self, report: Report) -> Result<String, BencherError> {
        if let Some(backend) = &self.backend {
            backend.output(report)
        } else {
            let mut reports = Reports::new();
            reports.add(report);
            let reports_str = serde_json::to_string(&reports)?;
            Ok(reports_str)
        }
    }
}
