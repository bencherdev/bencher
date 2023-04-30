use std::process::Command;

use anyhow::Context as _;
use clap::Parser;

use crate::build_ebpf::{build_ebpf, Architecture, Options as BuildOptions};

#[derive(Debug, Parser)]
pub struct Options {
    /// Set the endianness of the BPF target
    #[clap(default_value = "bpfel-unknown-none", long)]
    pub bpf_target: Architecture,
}

/// Build and run the project
pub fn bench(opts: Options) -> Result<(), anyhow::Error> {
    // build our ebpf program followed by our application
    build_ebpf(BuildOptions {
        target: opts.bpf_target,
        release: true,
    })
    .context("Error while building eBPF program")?;

    // run the command
    let status = Command::new("cargo")
        .args(["+nightly", "bench"])
        .status()
        .expect("failed to run the command");

    if !status.success() {
        anyhow::bail!("Failed to run benchmarks");
    }

    Ok(())
}
