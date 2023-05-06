use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::Parser;
use log::info;
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

    let shutdown = Arc::new(AtomicBool::new(false));
    let ebpf_shutdown = shutdown.clone();
    ebpf::run(&opt.iface, ebpf_shutdown).await?;

    info!("Waiting for Ctrl-C...");
    signal::ctrl_c().await?;
    info!("Exiting...");
    shutdown.store(true, Ordering::Relaxed);

    Ok(())
}
