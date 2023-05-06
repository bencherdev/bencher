use std::os::fd::AsRawFd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Context;
use aya::programs::{Xdp, XdpFlags};
use aya::{include_bytes_aligned, Bpf};
use aya_log::BpfLogger;
use ebpf_common::SourceAddr;
use log::{info, warn};

pub struct Process {
    pub pid: u32,
    pub prog_fd: i32,
    pub handle: tokio::task::JoinHandle<Result<(), anyhow::Error>>,
}

pub async fn run(iface: &str, shutdown: Arc<AtomicBool>) -> Result<Process, anyhow::Error> {
    env_logger::init();

    #[cfg(debug_assertions)]
    let mut bpf = Bpf::load(include_bytes_aligned!(
        "../../target/bpfel-unknown-none/debug/ebpf"
    ))?;
    #[cfg(not(debug_assertions))]
    let mut bpf = Bpf::load(include_bytes_aligned!(
        "../../target/bpfel-unknown-none/release/ebpf"
    ))?;
    if let Err(e) = BpfLogger::init(&mut bpf) {
        // This can happen if you remove all log statements from your eBPF program.
        warn!("failed to initialize eBPF logger: {}", e);
    }
    let program: &mut Xdp = bpf.program_mut("fun_xdp").unwrap().try_into()?;
    program.load()?;
    program.attach(iface, XdpFlags::default())
        .context("failed to attach the XDP program with default flags - try changing XdpFlags::default() to XdpFlags::SKB_MODE")?;

    let pid = std::process::id();
    let prog_fd = bpf.program("fun_xdp").unwrap().fd().unwrap().as_raw_fd();
    info!("Process ID: {}", pid);
    info!("Program FD: {}", prog_fd);

    let handle = tokio::spawn(async move { spawn_agent(&mut bpf, shutdown).await });

    Ok(Process {
        pid,
        prog_fd,
        handle,
    })
}

async fn spawn_agent(bpf: &mut Bpf, shutdown: Arc<AtomicBool>) -> Result<(), anyhow::Error> {
    let mut xdp_map: aya::maps::Queue<_, SourceAddr> =
        aya::maps::Queue::try_from(bpf.map_mut("SOURCE_ADDR_QUEUE").unwrap()).unwrap();

    loop {
        while let Ok(source_addr) = xdp_map.pop(0) {
            info!("{:?}", source_addr);
        }
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
    }

    Ok(())
}
