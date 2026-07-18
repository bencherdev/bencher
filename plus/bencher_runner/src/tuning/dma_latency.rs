//! C-state control via the PM `QoS` interface.
//!
//! Writing a latency value (in microseconds) to `/dev/cpu_dma_latency` and
//! holding the file descriptor open asks the kernel to keep CPU exit latency
//! at or below that value. A value of 0 keeps all CPUs out of deep C-states,
//! removing wakeup latency jitter from benchmark runs. The constraint is
//! released automatically when the fd closes, so a crashed runner cannot
//! leave the host stuck in polling mode. This works on both x86 and ARM,
//! unlike per-cpu `cpuidle/state*/disable` writes.

use camino::Utf8Path;

use super::TuningGuard;

/// PM `QoS` device node.
pub(super) const CPU_DMA_LATENCY: &str = "/dev/cpu_dma_latency";

/// The kernel expects a native-endian i32 latency in microseconds.
/// Only the value 0 is supported here, whose byte representation is
/// endianness-independent; any future non-zero latency must be encoded
/// with `i32::to_ne_bytes` instead of a literal array.
const ZERO_LATENCY: [u8; 4] = [0; 4];

/// Request a maximum CPU exit latency of 0 us and hold the constraint.
///
/// The opened file is pushed onto the guard so the constraint stays active
/// until the guard drops. Missing device nodes (e.g., kernels without
/// `CONFIG_CPU_IDLE`) are skipped with an informational message.
pub(super) fn hold_dma_latency(guard: &mut TuningGuard, path: &Utf8Path) {
    use std::io::Write as _;

    if !path.exists() {
        println!("  Tuning: C-states - skipped (path not found)");
        return;
    }

    let mut file = match std::fs::OpenOptions::new().write(true).open(path) {
        Ok(file) => file,
        Err(e) => {
            println!("  Tuning: C-states - skipped (open failed: {e})");
            return;
        },
    };

    if let Err(e) = file.write_all(&ZERO_LATENCY) {
        println!("  Tuning: C-states - skipped (write failed: {e})");
        return;
    }

    println!("  Tuning: C-states - max exit latency held at 0 us (released on exit)");
    guard.held_fds.push(file);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_guard() -> TuningGuard {
        TuningGuard {
            saved: Vec::new(),
            held_fds: Vec::new(),
        }
    }

    #[test]
    fn holds_fd_and_writes_zero_latency() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cpu_dma_latency");
        std::fs::write(&path, [0xffu8; 4]).unwrap();

        let mut guard = empty_guard();
        hold_dma_latency(&mut guard, Utf8Path::new(path.to_str().unwrap()));

        assert_eq!(guard.held_fds.len(), 1);
        // The write starts at offset 0 and covers all 4 original bytes.
        let contents = std::fs::read(&path).unwrap();
        assert_eq!(contents, ZERO_LATENCY);
    }

    #[test]
    fn skips_missing_path() {
        let mut guard = empty_guard();
        hold_dma_latency(&mut guard, Utf8Path::new("/nonexistent/cpu_dma_latency"));
        assert!(guard.held_fds.is_empty());
    }

    #[test]
    fn guard_drop_releases_fd() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cpu_dma_latency");
        std::fs::write(&path, []).unwrap();

        let mut guard = empty_guard();
        hold_dma_latency(&mut guard, Utf8Path::new(path.to_str().unwrap()));
        assert_eq!(guard.held_fds.len(), 1);
        // Dropping the guard must not panic and releases the fd.
        drop(guard);
    }
}
