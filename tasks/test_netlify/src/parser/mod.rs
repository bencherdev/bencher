use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct TaskTask {
    #[clap(subcommand)]
    pub sub: TaskSub,
}

#[allow(variant_size_differences, clippy::large_enum_variant)]
#[derive(Subcommand, Debug)]
pub enum TaskSub {
    Dev(TaskTestNetlify),
    Prod(TaskTestNetlify),
}

#[derive(Parser, Debug)]
pub struct TaskTestNetlify {
    pub ref_name: String,
}
