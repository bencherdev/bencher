//! VMM sandboxing with seccomp and capability dropping.
//!
//! This module provides security hardening for the VMM process by:
//!
//! 1. **Dropping Linux capabilities** - Removes all capabilities except CAP_NET_ADMIN
//!    (needed for some KVM operations on certain systems).
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
    BpfMap, SeccompAction, SeccompFilter, SeccompRule, TargetArch,
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

    // Keep CAP_NET_ADMIN for vsock on some systems, drop everything else
    let caps_to_keep = vec![Capability::CAP_NET_ADMIN];

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

    // Compile the filter for the current architecture
    #[cfg(target_arch = "x86_64")]
    let arch = TargetArch::x86_64;
    #[cfg(target_arch = "aarch64")]
    let arch = TargetArch::aarch64;

    let mut filters: BTreeMap<String, SeccompFilter> = BTreeMap::new();
    filters.insert("vmm".to_string(), filter);

    let bpf_map: BpfMap = seccompiler::compile_from_filters(&filters, arch)
        .map_err(|e| VmmError::Sandbox(format!("Failed to compile seccomp filter: {e}")))?;

    let bpf_prog = bpf_map
        .get("vmm")
        .ok_or_else(|| VmmError::Sandbox("Missing vmm filter".to_string()))?;

    // Apply the filter - this is irreversible
    seccompiler::apply_filter(bpf_prog)
        .map_err(|e| VmmError::Sandbox(format!("Failed to apply seccomp filter: {e}")))?;

    Ok(())
}

/// Build the seccomp filter with allowed syscalls.
fn build_seccomp_filter() -> Result<SeccompFilter, VmmError> {
    use libc::*;

    // Define allowed syscalls
    // These are the minimal syscalls needed for KVM vCPU execution
    let mut rules: Vec<(i64, Vec<SeccompRule>)> = Vec::new();

    // Helper to add a simple allow rule
    let allow = |syscall: i64| (syscall, vec![SeccompRule::new(vec![]).unwrap()]);

    // === KVM operations ===
    rules.push(allow(SYS_ioctl)); // KVM ioctls

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
    #[cfg(target_arch = "x86_64")]
    rules.push(allow(SYS_newfstatat)); // File status (newer variant)
    #[cfg(target_arch = "aarch64")]
    rules.push(allow(SYS_newfstatat));

    // === I/O operations ===
    rules.push(allow(SYS_ppoll)); // Polling (for event handling)
    rules.push(allow(SYS_epoll_wait)); // epoll waiting
    #[cfg(target_arch = "x86_64")]
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

    // === Signal handling ===
    rules.push(allow(SYS_rt_sigaction)); // Signal handlers
    rules.push(allow(SYS_rt_sigprocmask)); // Signal mask
    rules.push(allow(SYS_rt_sigreturn)); // Return from signal
    rules.push(allow(SYS_sigaltstack)); // Alternate signal stack

    // === Process control ===
    rules.push(allow(SYS_exit)); // Exit thread
    rules.push(allow(SYS_exit_group)); // Exit process
    rules.push(allow(SYS_getpid)); // Get process ID (for logging)
    rules.push(allow(SYS_getrandom)); // Random numbers (for crypto/vsock)

    // === Memory info ===
    rules.push(allow(SYS_prctl)); // Process control (for security features)

    // Build the filter map
    let rules_map: BTreeMap<i64, Vec<SeccompRule>> = rules.into_iter().collect();

    // Default action: kill the process if a disallowed syscall is attempted
    SeccompFilter::new(
        rules_map,
        SeccompAction::KillProcess,  // Kill on violation
        SeccompAction::Allow,        // Allow matched rules
        TargetArch::x86_64,          // Will be overridden when compiling
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
