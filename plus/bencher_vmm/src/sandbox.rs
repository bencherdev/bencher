//! VMM sandboxing with seccomp and capability dropping.
//!
//! This module provides security hardening for the VMM process by:
//!
//! 1. **Dropping Linux capabilities** - Removes all Linux capabilities for maximum
//!    security isolation.
//!
//! 2. **Applying seccomp filters** - Restricts the syscalls the VMM can make to only
//!    those required for KVM operation. This limits the damage if a guest exploits
//!    a bug in the VMM (e.g., virtio parsing).
//!
//! The sandbox is applied in two phases:
//! - `drop_capabilities()`: Called after KVM setup but before VM execution
//! - `apply_seccomp()`: Called just before entering the VM run loop

use std::collections::BTreeMap;

use seccompiler::{
    BpfProgram, SeccompAction, SeccompCmpArgLen, SeccompCmpOp, SeccompCondition, SeccompFilter,
    SeccompRule, TargetArch,
};

use crate::error::VmmError;

/// Drop all Linux capabilities.
///
/// This should be called after opening KVM and setting up the VM,
/// but before running the vCPU loop.
///
/// # Errors
///
/// Returns an error if capability dropping fails.
pub fn drop_capabilities() -> Result<(), VmmError> {
    use caps::{CapSet, Capability};

    // Drop ALL capabilities - testing showed vsock works without CAP_NET_ADMIN
    let caps_to_keep: Vec<Capability> = vec![];

    // Get current permitted capabilities
    let permitted = caps::read(None, CapSet::Permitted)
        .map_err(|e| VmmError::Sandbox(format!("Failed to read capabilities: {e}")))?;

    // Drop capabilities we don't need
    for cap in permitted.iter() {
        if !caps_to_keep.contains(cap) {
            caps::drop(None, CapSet::Permitted, *cap)
                .map_err(|e| VmmError::Sandbox(format!("Failed to drop {cap:?}: {e}")))?;
            caps::drop(None, CapSet::Effective, *cap)
                .map_err(|e| VmmError::Sandbox(format!("Failed to drop {cap:?}: {e}")))?;
            caps::drop(None, CapSet::Inheritable, *cap)
                .map_err(|e| VmmError::Sandbox(format!("Failed to drop {cap:?}: {e}")))?;
        }
    }

    Ok(())
}

/// Apply seccomp filters to restrict syscalls.
///
/// This creates a strict allowlist of syscalls needed for KVM operation.
/// Any syscall not in the list will cause the process to be killed.
///
/// # Safety
///
/// This function must be called after all setup is complete (file opens,
/// memory mapping, thread creation) as those syscalls will be blocked.
///
/// # Errors
///
/// Returns an error if seccomp filter application fails.
pub fn apply_seccomp() -> Result<(), VmmError> {
    let filter = build_seccomp_filter()?;

    // Convert the filter to BPF program
    let bpf_prog: BpfProgram = filter
        .try_into()
        .map_err(|e| VmmError::Sandbox(format!("Failed to compile seccomp filter: {e}")))?;

    // Apply the filter - this is irreversible
    seccompiler::apply_filter(&bpf_prog)
        .map_err(|e| VmmError::Sandbox(format!("Failed to apply seccomp filter: {e}")))?;

    Ok(())
}

/// Build the seccomp filter with allowed syscalls.
///
/// Syscalls are restricted with argument-level filtering where practical:
/// - `ioctl`: Only KVM ioctls (type 0xAE) are allowed
/// - `socket`: Only AF_UNIX sockets are allowed (no network)
/// - `prctl`: Only PR_SET_NAME is allowed (used by Rust runtime)
fn build_seccomp_filter() -> Result<SeccompFilter, VmmError> {
    use libc::*;

    // Define allowed syscalls
    // These are the minimal syscalls needed for KVM vCPU execution
    let mut rules: Vec<(i64, Vec<SeccompRule>)> = Vec::new();

    // Helper to add a simple allow rule (empty Vec means unconditional allow)
    let allow = |syscall: i64| (syscall, vec![]);

    // Helper for creating seccomp errors
    let seccomp_err =
        |msg: &str, e| VmmError::Sandbox(format!("Failed to create seccomp {msg}: {e}"));

    // === KVM operations ===
    // Restrict ioctl to KVM type only (type byte = 0xAE in bits 8-15)
    // This prevents abuse of ioctl for non-KVM device manipulation.
    // The ioctl request number encoding: bits 8-15 = type, bits 0-7 = nr
    rules.push((
        SYS_ioctl,
        vec![SeccompRule::new(vec![SeccompCondition::new(
            1, // arg1 = ioctl request number
            SeccompCmpArgLen::Dword,
            SeccompCmpOp::MaskedEq(0x0000_FF00),
            0x0000_AE00, // KVM type = 0xAE
        )
        .map_err(|e| seccomp_err("ioctl condition", e))?])
        .map_err(|e| seccomp_err("ioctl rule", e))?],
    ));

    // === Memory operations ===
    rules.push(allow(SYS_mmap)); // Memory mapping (needed for KVM memory regions)
    rules.push(allow(SYS_munmap)); // Memory unmapping
    rules.push(allow(SYS_mprotect)); // Memory protection changes
    rules.push(allow(SYS_madvise)); // Memory hints
    rules.push(allow(SYS_brk)); // Heap allocation

    // === File operations (for virtio, vsock) ===
    rules.push(allow(SYS_read)); // Read from vsock, files
    rules.push(allow(SYS_write)); // Write to vsock, serial
    rules.push(allow(SYS_close)); // Close file descriptors
    rules.push(allow(SYS_fstat)); // File status (used by some operations)
    rules.push(allow(SYS_unlink)); // Remove files (cleanup)
    rules.push(allow(SYS_unlinkat)); // Remove files (at directory fd)
    rules.push(allow(SYS_openat)); // Open files
    rules.push(allow(SYS_lseek)); // Seek in files
    rules.push(allow(SYS_readv)); // Scatter read
    rules.push(allow(SYS_writev)); // Gather write
    rules.push(allow(SYS_pread64)); // Positional read
    rules.push(allow(SYS_pwrite64)); // Positional write
    #[cfg(target_arch = "x86_64")]
    rules.push(allow(SYS_newfstatat)); // File status (newer variant)
    #[cfg(target_arch = "aarch64")]
    rules.push(allow(SYS_newfstatat));
    rules.push(allow(SYS_statx)); // Extended file status
    rules.push(allow(SYS_getdents64)); // Read directory entries

    // === Socket operations (for vsock) ===
    // Restrict socket creation to AF_UNIX only - prevents network access from VMM
    rules.push((
        SYS_socket,
        vec![SeccompRule::new(vec![SeccompCondition::new(
            0, // arg0 = domain
            SeccompCmpArgLen::Dword,
            SeccompCmpOp::Eq,
            AF_UNIX as u64,
        )
        .map_err(|e| seccomp_err("socket condition", e))?])
        .map_err(|e| seccomp_err("socket rule", e))?],
    ));
    rules.push(allow(SYS_bind)); // Bind socket
    rules.push(allow(SYS_listen)); // Listen on socket
    rules.push(allow(SYS_accept4)); // Accept connection
    rules.push(allow(SYS_connect)); // Connect socket
    rules.push(allow(SYS_sendto)); // Send data
    rules.push(allow(SYS_recvfrom)); // Receive data
    rules.push(allow(SYS_sendmsg)); // Send message
    rules.push(allow(SYS_recvmsg)); // Receive message
    rules.push(allow(SYS_shutdown)); // Shutdown socket
    rules.push(allow(SYS_getsockopt)); // Get socket options
    rules.push(allow(SYS_setsockopt)); // Set socket options

    // === I/O operations ===
    rules.push(allow(SYS_ppoll)); // Polling (for event handling)
    #[cfg(target_arch = "x86_64")]
    rules.push(allow(SYS_epoll_wait)); // epoll waiting (x86_64 only)
    rules.push(allow(SYS_epoll_pwait)); // epoll with signal mask
    rules.push(allow(SYS_epoll_pwait2)); // epoll with timespec
    rules.push(allow(SYS_eventfd2)); // Event file descriptors
    rules.push(allow(SYS_fcntl)); // File descriptor control

    // === Timing (for timeouts) ===
    rules.push(allow(SYS_clock_gettime)); // Get current time
    rules.push(allow(SYS_nanosleep)); // Sleep (timeout thread)
    rules.push(allow(SYS_clock_nanosleep)); // Sleep with clock

    // === Threading (for multi-vCPU) ===
    rules.push(allow(SYS_futex)); // Mutex/condvar operations
    rules.push(allow(SYS_set_robust_list)); // Thread setup
    rules.push(allow(SYS_rseq)); // Restartable sequences
    rules.push(allow(SYS_sched_yield)); // Yield CPU
    rules.push(allow(SYS_sched_getaffinity)); // Get CPU affinity
    rules.push(allow(SYS_gettid)); // Get thread ID
    rules.push(allow(SYS_clone3)); // Thread creation (modern)

    // === Signal handling ===
    rules.push(allow(SYS_rt_sigaction)); // Signal handlers
    rules.push(allow(SYS_rt_sigprocmask)); // Signal mask
    rules.push(allow(SYS_rt_sigreturn)); // Return from signal
    rules.push(allow(SYS_sigaltstack)); // Alternate signal stack

    // === Process control ===
    rules.push(allow(SYS_exit)); // Exit thread
    rules.push(allow(SYS_exit_group)); // Exit process
    rules.push(allow(SYS_getpid)); // Get process ID (for logging)
    rules.push(allow(SYS_kill)); // Send signal (timeout uses SIGALRM to interrupt vCPU)
    rules.push(allow(SYS_tgkill)); // Thread-directed signal (used by some kill implementations)
    rules.push(allow(SYS_getrandom)); // Random numbers (for crypto/vsock)

    // === Process info ===
    // Restrict prctl to PR_SET_NAME only (used by Rust runtime for thread naming)
    rules.push((
        SYS_prctl,
        vec![SeccompRule::new(vec![SeccompCondition::new(
            0, // arg0 = option
            SeccompCmpArgLen::Dword,
            SeccompCmpOp::Eq,
            PR_SET_NAME as u64,
        )
        .map_err(|e| seccomp_err("prctl condition", e))?])
        .map_err(|e| seccomp_err("prctl rule", e))?],
    ));

    // Build the filter map
    let rules_map: BTreeMap<i64, Vec<SeccompRule>> = rules.into_iter().collect();

    // Get target architecture
    #[cfg(target_arch = "x86_64")]
    let arch = TargetArch::x86_64;
    #[cfg(target_arch = "aarch64")]
    let arch = TargetArch::aarch64;

    // Default action: kill the process if a disallowed syscall is attempted
    SeccompFilter::new(
        rules_map,
        SeccompAction::KillProcess, // Kill on violation
        SeccompAction::Allow,       // Allow matched rules
        arch,
    )
    .map_err(|e| VmmError::Sandbox(format!("Failed to create seccomp filter: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drop_capabilities() {
        // This test may fail if run without CAP_SETPCAP
        // In CI, run as root or with appropriate capabilities
        if caps::has_cap(None, caps::CapSet::Permitted, caps::Capability::CAP_SETPCAP)
            .unwrap_or(false)
        {
            drop_capabilities().expect("Failed to drop capabilities");
        }
    }

    #[test]
    fn test_build_seccomp_filter() {
        // Just test that the filter builds without error
        build_seccomp_filter().expect("Failed to build seccomp filter");
    }

    // Note: Can't easily test apply_seccomp() because it's irreversible
    // and would affect subsequent tests
}
