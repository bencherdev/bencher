use clap::Parser;

#[derive(Parser, Debug)]
pub struct TaskStats {
    /// Stats JSON
    pub stats: String,
}
