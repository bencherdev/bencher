use std::os::fd::AsRawFd;

use anyhow::Context;
use aya::programs::{Xdp, XdpFlags};
use aya::{include_bytes_aligned, Bpf};
use aya_log::BpfLogger;
use clap::Parser;
use ebpf_common::SourceAddr;
use log::{info, warn};
use tokio::signal;

#[derive(Debug, Parser)]
struct Opt {
    // `ip link show`
    #[clap(short, long, default_value = "ens160")]
    iface: String,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let opt = Opt::parse();

    ebpf::run(&opt.iface).await?;

    info!("Waiting for Ctrl-C...");
    signal::ctrl_c().await?;
    info!("Exiting...");

    Ok(())
}
