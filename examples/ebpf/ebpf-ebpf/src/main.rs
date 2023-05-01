#![no_std]
#![no_main]

use aya_bpf::{
    bindings::xdp_action,
    macros::{map, xdp},
    maps::Queue,
    programs::XdpContext,
};
use aya_log_ebpf::{error, info};
use ebpf_common::SourceAddr;
use network_types::{
    eth::{EthHdr, EtherType},
    ip::Ipv4Hdr,
};

#[map]
pub static mut SOURCE_ADDR_QUEUE: Queue<SourceAddr> = Queue::with_max_entries(1024, 0);

#[xdp(name = "fun_xdp")]
pub fn fun_xdp(ctx: XdpContext) -> u32 {
    match try_fun_xdp(&ctx) {
        Ok(ret) => ret,
        Err(_) => xdp_action::XDP_ABORTED,
    }
}

fn try_fun_xdp(ctx: &XdpContext) -> Result<u32, ()> {
    let eth_hdr: *const EthHdr = unsafe { ptr_at(ctx, 0)? };

    unsafe {
        let EtherType::Ipv4 = (*eth_hdr).ether_type else {
            return Ok(xdp_action::XDP_PASS);
        };
    }

    let ipv4_hdr: *const Ipv4Hdr = unsafe { ptr_at(ctx, EthHdr::LEN)? };
    let source_addr = unsafe { (*ipv4_hdr).src_addr };

    // v0
    info!(ctx, "IPv4 Source Address: {}", source_addr);

    // v1
    let _opt_source_addr = SourceAddr::new_v1(source_addr);

    // v2
    let _opt_source_addr = SourceAddr::new_v2(source_addr);

    // v3
    let _opt_source_addr = SourceAddr::new_v3(source_addr);

    // v4
    let opt_source_addr = SourceAddr::new_v4(source_addr);

    if let Some(source_addr) = opt_source_addr {
        unsafe {
            if let Err(e) = SOURCE_ADDR_QUEUE.push(&source_addr, 0) {
                error!(ctx, "Failed to push source address into queue: {}", e);
            }
        }
    }

    Ok(xdp_action::XDP_PASS)
}

#[inline(always)]
unsafe fn ptr_at<T>(ctx: &XdpContext, offset: usize) -> Result<*const T, ()> {
    let start = ctx.data();
    let end = ctx.data_end();
    let len = core::mem::size_of::<T>();

    if start + offset + len > end {
        return Err(());
    }

    Ok((start + offset) as _)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
