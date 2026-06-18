use bencher_json::{Jwt, Url};
use clap::{Parser, ValueEnum};

#[derive(Parser, Debug, Default)]
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

#[derive(Parser, Debug, Default)]
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
#[expect(
    clippy::enum_variant_names,
    reason = "Rust prefix groups language-specific examples"
)]
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
#[expect(
    clippy::doc_markdown,
    reason = "doc comments contain URLs used as variant descriptions"
)]
pub enum TaskTestEnvironment {
    #[default]
    /// http://localhost:6610
    Ci,
    /// http://localhost:6610
    Localhost,
    /// Docker http://localhost:6610
    Docker,
    /// https://dev.api.bencher.dev
    Dev,
    /// https://test.api.bencher.dev
    Test,
    /// https://api.bencher.dev
    Prod,
}
