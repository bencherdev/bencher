use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct TaskTask {
    #[clap(subcommand)]
    pub sub: Option<TaskSub>,
}

#[derive(Subcommand, Debug)]
pub enum TaskSub {
    /// Run the full test (default)
    Test(TaskTest),
    /// Only download/build the kernel
    Kernel(TaskKernel),
    /// Only create the OCI image
    Oci(TaskOci),
    /// Clean up test artifacts
    Clean(TaskClean),
}

#[derive(Parser, Debug)]
pub struct TaskTest {}

#[derive(Parser, Debug)]
pub struct TaskKernel {}

#[derive(Parser, Debug)]
pub struct TaskOci {}

#[derive(Parser, Debug)]
pub struct TaskClean {}
