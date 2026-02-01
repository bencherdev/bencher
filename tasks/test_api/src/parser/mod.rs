use clap::{Parser, Subcommand};

mod oci;
mod test;

pub use oci::TaskOci;
pub use test::{TaskExample, TaskExamples, TaskSeedTest, TaskSmokeTest, TaskTestEnvironment};

#[derive(Parser, Debug)]
pub struct TaskTask {
    #[clap(subcommand)]
    pub sub: TaskSub,
}

#[derive(Subcommand, Debug)]
pub enum TaskSub {
    /// Run Seed test
    Seed(TaskSeedTest),
    // Run Example(s)
    Examples(TaskExamples),
    /// Run smoke test
    Smoke(TaskSmokeTest),
    /// Run OCI Distribution Spec conformance tests
    Oci(TaskOci),
}
