use clap::{Parser, Subcommand};

#[cfg(feature = "plus")]
mod oci;
#[cfg(feature = "plus")]
mod runner;
mod test;

#[cfg(feature = "plus")]
pub use oci::TaskOci;
#[cfg(feature = "plus")]
pub use runner::TaskRunner;
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
    #[cfg(feature = "plus")]
    Oci(TaskOci),
    /// Run runner smoke test (docker pull/push + bencher run --image)
    #[cfg(feature = "plus")]
    Runner(TaskRunner),
}
