//! Test utilities for the `firecracker` unit tests.

use std::os::unix::thread::JoinHandleExt as _;
use std::thread::JoinHandle;
use std::time::Duration;

use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction};

/// Signals `thread` through a handler that does nothing, often enough that
/// one signal lands while it waits in a read.
pub(super) fn interrupt<T>(thread: &JoinHandle<T>) {
    let action = SigAction::new(
        SigHandler::Handler(ignore),
        SaFlags::empty(),
        SigSet::empty(),
    );
    #[expect(unsafe_code, reason = "sigaction requires unsafe FFI")]
    // SAFETY: the handler does nothing, which is async-signal-safe.
    let installed = unsafe { sigaction(Signal::SIGUSR2, &action) };
    installed.unwrap();
    for _ in 0..50 {
        #[expect(unsafe_code, reason = "pthread_kill requires unsafe FFI")]
        // SAFETY: the thread is not joined yet, so its id is still valid.
        let sent = unsafe { libc::pthread_kill(thread.as_pthread_t(), libc::SIGUSR2) };
        assert_eq!(sent, 0, "pthread_kill must succeed");
        std::thread::sleep(Duration::from_millis(1));
    }
}

extern "C" fn ignore(_signal: libc::c_int) {}
