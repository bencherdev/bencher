use std::convert::TryFrom;

use ::clap::Parser;
use email_address_parser::EmailAddress;
use reports::Metrics;
use reports::Report;
use reports::Testbed;

pub mod adapter;
pub mod backend;
pub mod benchmark;
pub mod clap;

use crate::cli::clap::CliBencher;
use crate::cli::clap::CliTestbed;
use crate::BencherError;
use adapter::Adapter;
use backend::Backend;
use benchmark::Benchmark;
use benchmark::BenchmarkOutput;

pub const BENCHER_URL: &str = "https://api.bencher.dev";
const BENCHER_TOKEN: &str = "BENCHER_TOKEN";

#[derive(Debug)]
pub struct Bencher {
    benchmark: Benchmark,
    adapter: Adapter,
    email: EmailAddress,
    token: String,
    project: Option<String>,
    testbed: Testbed,
    backend: Backend,
}

impl TryFrom<CliBencher> for Bencher {
    type Error = BencherError;

    fn try_from(bencher: CliBencher) -> Result<Self, Self::Error> {
        Ok(Self {
            benchmark: Benchmark::try_from(bencher.benchmark)?,
            adapter: Adapter::from(bencher.adapter),
            email: map_email(bencher.email)?,
            token: map_token(bencher.token)?,
            project: bencher.project,
            testbed: bencher.testbed.into(),
            backend: Backend::try_from(bencher.backend)?,
        })
    }
}

fn map_email(email: String) -> Result<EmailAddress, BencherError> {
    EmailAddress::parse(&email, None).ok_or(BencherError::Email(email))
}

fn map_token(token: Option<String>) -> Result<String, BencherError> {
    // TODO add first pass token validation
    if let Some(token) = token {
        return Ok(token);
    }
    if let Ok(token) = std::env::var(BENCHER_TOKEN) {
        return Ok(token);
    };
    Err(BencherError::Token)
}

impl Bencher {
    pub fn new() -> Result<Self, BencherError> {
        let args = CliBencher::parse();
        Self::try_from(args)
    }

    pub fn run(&self) -> Result<BenchmarkOutput, BencherError> {
        self.benchmark.run()
    }

    pub fn convert(&self, output: BenchmarkOutput) -> Result<Metrics, BencherError> {
        self.adapter.convert(output)
    }

    pub async fn send(&self, metrics: Metrics) -> Result<(), BencherError> {
        let report = Report::new(
            self.email.to_string(),
            self.token.clone(),
            self.project.clone(),
            self.testbed.clone(),
            metrics,
        );
        self.backend.send(report).await
    }
}

impl Into<Testbed> for CliTestbed {
    fn into(self) -> Testbed {
        Testbed {
            name: self.testbed,
            os: self.os,
            os_version: self.os_version,
            cpu: self.cpu,
            ram: self.ram,
            disk: self.disk,
            arch: self.arch,
        }
    }
}
