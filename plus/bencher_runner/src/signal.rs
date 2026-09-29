//! SIGINT and SIGTERM, turned into a flag the runner polls.

use std::sync::atomic::{AtomicBool, Ordering};

/// Set once SIGINT or SIGTERM arrives.
static STOP: AtomicBool = AtomicBool::new(false);

/// The flag the handlers set, for a job to poll as its cancel flag.
pub(crate) fn stop_flag() -> &'static AtomicBool {
    &STOP
}

/// Whether SIGINT or SIGTERM has arrived.
pub(crate) fn stop_requested() -> bool {
    STOP.load(Ordering::SeqCst)
}

/// Install handlers for SIGINT and SIGTERM that only set the flag.
#[cfg(target_os = "linux")]
pub(crate) fn install_handlers() {
    use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction};

    let handler = SigHandler::Handler(handle);
    let action = SigAction::new(handler, SaFlags::empty(), SigSet::empty());

    #[expect(
        unsafe_code,
        clippy::multiple_unsafe_ops_per_block,
        reason = "sigaction requires unsafe FFI"
    )]
    // SAFETY: `handle` only performs `AtomicBool::store` with
    // `Ordering::SeqCst`, which is async-signal-safe per POSIX.
    unsafe {
        _ = sigaction(Signal::SIGINT, &action);
        _ = sigaction(Signal::SIGTERM, &action);
    }
}

/// Install handlers for SIGINT and SIGTERM that only set the flag.
///
/// Uses `libc::signal()` directly since `nix` is not available on macOS.
#[cfg(not(target_os = "linux"))]
pub(crate) fn install_handlers() {
    #[expect(
        unsafe_code,
        clippy::fn_to_numeric_cast_any,
        reason = "libc::signal requires unsafe FFI and handler cast"
    )]
    // SAFETY: `handle` only performs `AtomicBool::store` with
    // `Ordering::SeqCst`, which is async-signal-safe per POSIX.
    unsafe {
        libc::signal(libc::SIGINT, handle as *const () as libc::sighandler_t);
    }
    #[expect(
        unsafe_code,
        clippy::fn_to_numeric_cast_any,
        reason = "libc::signal requires unsafe FFI and handler cast"
    )]
    // SAFETY: the same async-signal-safe handler, registered for SIGTERM.
    unsafe {
        libc::signal(libc::SIGTERM, handle as *const () as libc::sighandler_t);
    }
}

extern "C" fn handle(_sig: libc::c_int) {
    STOP.store(true, Ordering::SeqCst);
}
