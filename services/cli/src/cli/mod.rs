use std::convert::TryFrom;

use ::clap::Parser;
use reports::{Report, Reports};

pub mod adapter;
pub mod backend;
pub mod benchmark;
pub mod clap;

use crate::cli::clap::CliBencher;
use crate::BencherError;
use adapter::Adapter;
use backend::Backend;
use benchmark::Benchmark;
use benchmark::BenchmarkOutput;

#[derive(Debug)]
pub struct Bencher {
    benchmark: Benchmark,
    adapter: Adapter,
    backend: Backend,
}

impl TryFrom<CliBencher> for Bencher {
    type Error = BencherError;

    fn try_from(bencher: CliBencher) -> Result<Self, Self::Error> {
        Ok(Self {
            benchmark: Benchmark::try_from(bencher.benchmark)?,
            adapter: Adapter::from(bencher.adapter),
            backend: Backend::from(bencher.backend),
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

    pub fn send(&self, report: Report) -> Result<(), BencherError> {
        self.backend.send(report)
    }
}
