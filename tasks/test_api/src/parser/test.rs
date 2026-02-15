use bencher_json::{Jwt, Url};
use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
pub struct TaskSeedTest {
    /// Test API URL
    #[clap(long)]
    pub url: Option<Url>,
    /// Admin test token
    #[clap(long)]
    pub admin_token: Option<Jwt>,
    /// Test token
    #[clap(long)]
    pub token: Option<Jwt>,
    /// Is Bencher Cloud
    #[clap(long)]
    pub is_bencher_cloud: bool,
    /// Running outside of a git repo
    #[clap(long)]
    pub no_git: bool,
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
#[expect(clippy::enum_variant_names)]
pub enum TaskExample {
    /// Rust libtest bench
    RustBench,
    /// Rust Criterion
    RustCriterion,
    /// Rust Iai
    RustIai,
    /// Rust Iai Callgrind
    RustIaiCallgrind,
    /// Rust Custom Benchmark Harness
    RustCustom,
}

#[derive(Parser, Debug)]
pub struct TaskSmokeTest {
    /// Test environment
    pub environment: Option<TaskTestEnvironment>,
}

/// Template kind
#[derive(ValueEnum, Debug, Clone, Copy, Default)]
#[clap(rename_all = "snake_case")]
#[expect(clippy::doc_markdown)]
pub enum TaskTestEnvironment {
    #[default]
    /// https://localhost:61016
    Ci,
    /// https://localhost:61016
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
