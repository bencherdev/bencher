use std::convert::TryFrom;

use ::clap::Parser;
use reports::{Report, Reports};

pub mod adapter;
pub mod backend;
pub mod benchmark;
pub mod clap;
pub mod output;

use crate::cli::clap::CliBencher;
use crate::BencherError;
use adapter::Adapter;
use backend::Backend;
use benchmark::Benchmark;
use benchmark::BenchmarkOutput;
use output::Output;

#[derive(Debug)]
pub struct Bencher {
    benchmark: Benchmark,
    adapter: Adapter,
    backend: Option<Backend>,
    output: Output,
}

impl TryFrom<CliBencher> for Bencher {
    type Error = BencherError;

    fn try_from(bencher: CliBencher) -> Result<Self, Self::Error> {
        Ok(Self {
            benchmark: Benchmark::try_from(bencher.benchmark)?,
            adapter: Adapter::from(bencher.adapter),
            backend: if let Some(backend) = bencher.backend {
                Some(Backend::from(backend))
            } else {
                None
            },
            output: Output::from(bencher.output),
        })
    }
}

impl Bencher {
    pub fn new() -> Result<Self, BencherError> {
        let args = CliBencher::parse();
        Self::try_from(args)
    }

    pub fn run(&self) -> Result<BenchmarkOutput, BencherError> {
        self.benchmark.run()
    }

    pub fn convert(&self, output: BenchmarkOutput) -> Result<Report, BencherError> {
        self.adapter.convert(output)
    }

    pub fn output(&self, report: Report) -> Result<(), BencherError> {
        let reports = if let Some(backend) = &self.backend {
            backend.output(report)?
        } else {
            let mut reports = Reports::new();
            reports.add(report);
            serde_json::to_string(&reports)?
        };
        println!("{reports}");
        self.output.open(&reports)
    }
}
