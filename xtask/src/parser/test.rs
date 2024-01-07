use bencher_json::{Jwt, Url};
use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
pub struct TaskSeedTest {
    /// Test API URL
    pub url: Option<Url>,
    /// Test token
    pub token: Option<Jwt>,
}

#[derive(Parser, Debug)]
pub struct TaskSmokeTest {
    /// Test environment
    pub environment: TaskTestEnvironment,
}

/// Template kind
#[derive(ValueEnum, Debug, Clone, Copy)]
#[clap(rename_all = "snake_case")]
pub enum TaskTestEnvironment {
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

#[derive(Parser, Debug)]
pub struct TaskNetlifyTest {
    /// Run devel tests
    #[clap(long)]
    pub dev: bool,
}
