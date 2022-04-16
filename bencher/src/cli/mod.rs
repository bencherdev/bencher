use std::convert::TryFrom;

use ::clap::Parser;

use crate::cli::clap::CliBencher;
use crate::error::CliError;
use adapter::Adapter;
use adapter::Report;
use backend::Backend;
use benchmark::Benchmark;
use benchmark::Output;

pub mod adapter;
pub mod backend;
pub mod benchmark;
pub mod clap;

#[derive(Debug)]
pub struct Bencher {
    benchmark: Benchmark,
    adapter: Adapter,
    backend: Option<Backend>,
}

impl TryFrom<CliBencher> for Bencher {
    type Error = CliError;

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
    pub fn new() -> Result<Self, CliError> {
        let args = CliBencher::parse();
        Self::try_from(args)
    }

    pub fn run(&self) -> Result<Output, CliError> {
        self.benchmark.run()
    }

    pub fn convert(&self, output: Output) -> Result<Report, CliError> {
        self.adapter.convert(output)
    }

    pub fn output(&self, report: Report) -> Result<(), CliError> {
        if let Some(backend) = &self.backend {
            backend.output(report)
        } else {
            let report = serde_json::to_string(&report)?;
            println!("{report}");
            Ok(())
        }
    }
}
