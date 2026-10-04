//! SIGINT and SIGTERM, turned into a flag the runner polls.

use std::sync::atomic::{AtomicBool, Ordering};

static STOP: AtomicBool = AtomicBool::new(false);

pub(crate) fn stop_flag() -> &'static AtomicBool {
    &STOP
}

pub(crate) fn stop_requested() -> bool {
    STOP.load(Ordering::SeqCst)
}

/// Every SIGINT or SIGTERM only sets the flag.
pub(crate) fn install_handlers() {
    install(false);
}

/// The first SIGINT or SIGTERM sets the flag, and a second of the same signal
/// kills the process at once; the next job's sweep reclaims what it leaves.
pub(crate) fn install_cancel_handlers() {
    install(true);
}

#[cfg(target_os = "linux")]
fn install(reset: bool) {
    use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction};

    let flags = if reset {
        SaFlags::SA_RESETHAND
    } else {
        SaFlags::empty()
    };
    let action = SigAction::new(SigHandler::Handler(handle), flags, SigSet::empty());

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

/// Uses `libc::sigaction()` directly since `nix` is not available on macOS.
#[cfg(not(target_os = "linux"))]
fn install(reset: bool) {
    let flags = if reset { libc::SA_RESETHAND } else { 0 };
    for signal in [libc::SIGINT, libc::SIGTERM] {
        #[expect(
            unsafe_code,
            clippy::multiple_unsafe_ops_per_block,
            reason = "sigaction requires unsafe FFI"
        )]
        // SAFETY: `handle` only performs `AtomicBool::store` with
        // `Ordering::SeqCst`, which is async-signal-safe per POSIX, and the
        // zeroed action is a valid empty mask with no other flags.
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = handle as *const () as libc::sighandler_t;
            action.sa_flags = flags;
            libc::sigaction(signal, &raw const action, std::ptr::null_mut());
        }
    }
}

extern "C" fn handle(_sig: libc::c_int) {
    STOP.store(true, Ordering::SeqCst);
}
