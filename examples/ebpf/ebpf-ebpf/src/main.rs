#![no_std]
#![no_main]

use aya_bpf::{bindings::xdp_action, macros::xdp, programs::XdpContext};
use aya_log_ebpf::info;
use network_types::{
    eth::{EthHdr, EtherType},
    ip::Ipv4Hdr,
};

#[xdp(name = "fizz_buzz_fibonacci")]
pub fn fizz_buzz_fibonacci(ctx: XdpContext) -> u32 {
    match try_fizz_buzz_fibonacci(&ctx) {
        Ok(ret) => ret,
        Err(_) => xdp_action::XDP_ABORTED,
    }
}

fn try_fizz_buzz_fibonacci(ctx: &XdpContext) -> Result<u32, ()> {
    info!(ctx, "Received a packet");
    let eth_hdr: *const EthHdr = unsafe { ptr_at(ctx, 0)? };

    unsafe {
        let EtherType::Ipv4 = (*eth_hdr).ether_type else {
            return Ok(xdp_action::XDP_PASS);
        };
    }

    let ipv4_hdr: *const Ipv4Hdr = unsafe { ptr_at(ctx, EthHdr::LEN)? };
    let source_addr = unsafe { (*ipv4_hdr).src_addr };

    info!(ctx, "IPv4 Source Address: {}", source_addr);

    // if not n % 7:
    //     return fibonacci(n)

    // response = ''
    // if not n % 3:
    //     response += 'Fizz'
    // if not n % 5:
    //     response += 'Buzz'
    // return response if response else None

    // if source_addr % 7 == 0 {

    //     return Ok(xdp_action::XDP_DROP);
    // }

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
