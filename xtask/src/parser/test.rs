use bencher_json::{Jwt, Url};
use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
pub struct TaskSeedTest {
    /// Test API URL
    #[clap(long)]
    pub url: Option<Url>,
    /// Test token
    #[clap(long)]
    pub token: Option<Jwt>,
}

#[derive(Parser, Debug)]
pub struct TaskExamples {
    /// Test API URL
    #[clap(long)]
    pub url: Option<Url>,
    /// Test token
    #[clap(long)]
    pub token: Option<Jwt>,
    /// Example to run (default: all)
    pub example: Option<TaskExample>,
}

/// Template kind
#[derive(ValueEnum, Debug, Clone, Copy)]
#[clap(rename_all = "snake_case")]
pub enum TaskExample {
    /// Rust libtest bench
    RustBench,
}

#[derive(Parser, Debug)]
pub struct TaskSmokeTest {
    /// Test environment
    pub environment: Option<TaskTestEnvironment>,
}

/// Template kind
#[derive(ValueEnum, Debug, Clone, Copy, Default)]
#[clap(rename_all = "snake_case")]
pub enum TaskTestEnvironment {
    /// https://localhost:61016
    #[default]
    Localhost,
    /// Docker https://localhost:61016
    Docker,
    /// https://bencher-api-dev.fly.dev
    Dev,
    /// https://bencher-api-test.fly.dev
    Test,
    /// https://api.bencher.dev
    Prod,
}

#[derive(Parser, Debug)]
pub struct TaskNetlifyTest {
    /// Run devel tests
    #[clap(long)]
    pub dev: bool,
}
