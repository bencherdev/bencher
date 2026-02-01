//! Minimal vsock sender for guest VMs.
//!
//! This binary reads from stdin and sends the data to the host via vsock.
//! It's designed to be statically compiled and included in guest rootfs.
//!
//! Usage:
//!   echo "benchmark results" | vsock-send
//!   ./benchmark | vsock-send
//!
//! Build for guest VM:
//!   cargo build --release --target x86_64-unknown-linux-musl
//!   # or for aarch64:
//!   cargo build --release --target aarch64-unknown-linux-musl

#![expect(unsafe_code)]
#![expect(clippy::print_stderr)]

use std::io::{Read as _, Write as _};
use std::mem::size_of;
use std::os::unix::io::FromRawFd as _;

/// Host CID (always 2 for vsock).
const HOST_CID: u32 = 2;

/// Port for benchmark results.
const RESULTS_PORT: u32 = 5000;

/// `AF_VSOCK` address family.
const AF_VSOCK: libc::sa_family_t = 40;

/// `SOCK_STREAM` for connection-oriented sockets.
const SOCK_STREAM: libc::c_int = libc::SOCK_STREAM;

/// vsock address structure.
#[repr(C)]
struct SockaddrVm {
    family: libc::sa_family_t,
    reserved1: u16,
    port: u32,
    cid: u32,
    zero: [u8; 4],
}

fn main() {
    if let Err(e) = run() {
        eprintln!("vsock-send error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    // SAFETY: libc::socket is safe to call with valid arguments.
    // AF_VSOCK and SOCK_STREAM are valid socket types on Linux.
    let fd = unsafe { libc::socket(libc::c_int::from(AF_VSOCK), SOCK_STREAM, 0) };
    if fd < 0 {
        return Err(format!("socket() failed: {}", std::io::Error::last_os_error()));
    }

    // Connect to host
    let addr = SockaddrVm {
        family: AF_VSOCK,
        reserved1: 0,
        port: RESULTS_PORT,
        cid: HOST_CID,
        zero: [0; 4],
    };

    // SAFETY: We pass a valid socket fd and a properly initialized address structure.
    // The size matches the actual struct size.
    let ret = unsafe {
        libc::connect(
            fd,
            std::ptr::from_ref(&addr).cast(),
            socklen(size_of::<SockaddrVm>()),
        )
    };

    if ret < 0 {
        // SAFETY: fd is a valid file descriptor from socket().
        unsafe { libc::close(fd); }
        return Err(format!("connect() failed: {}", std::io::Error::last_os_error()));
    }

    // SAFETY: fd is a valid, connected socket file descriptor.
    // We take ownership and will close it when the File is dropped.
    let mut socket = unsafe { std::fs::File::from_raw_fd(fd) };
    let mut stdin = std::io::stdin().lock();
    let mut buffer = [0u8; 4096];

    loop {
        match stdin.read(&mut buffer) {
            Ok(0) => break, // EOF
            Ok(n) => {
                let data = buffer.get(..n).ok_or("buffer index out of bounds")?;
                if let Err(e) = socket.write_all(data) {
                    return Err(format!("write() failed: {e}"));
                }
            }
            Err(e) => {
                return Err(format!("read() failed: {e}"));
            }
        }
    }

    // Socket will be closed when dropped
    Ok(())
}

/// Convert a usize to `socklen_t` safely.
#[expect(clippy::cast_possible_truncation)]
const fn socklen(size: usize) -> libc::socklen_t {
    // socklen_t is u32, but size_of returns usize.
    // This is safe because struct sizes are always small.
    size as libc::socklen_t
}
