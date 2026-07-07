//! CPU isolation for non-sandboxed (host) execution.
//!
//! Mirrors the Firecracker path: the benchmark child process is placed in
//! a per-run cgroup with a cpuset restricted to the benchmark cores, so it
//! does not share cores with the runner's own threads. Attachment happens
//! race-free in the child between `fork` and `exec` (via `pre_exec`), so
//! the benchmark never runs a single instruction on the wrong cores.
//!
//! Everything here is best-effort: without root or cgroup v2 the cgroup
//! steps are skipped with a warning and only CPU affinity is applied.

#![cfg_attr(
    target_os = "linux",
    expect(
        clippy::print_stdout,
        clippy::print_stderr,
        reason = "isolation setup prints status and best-effort warnings"
    )
)]

use std::process::Command;

use crate::cpu::CpuLayout;
use crate::metrics::CgroupMetrics;

// ---------------------------------------------------------------------------
// Linux implementation
// ---------------------------------------------------------------------------

/// CPU isolation state for a single non-sandboxed run.
#[cfg(target_os = "linux")]
pub(crate) struct LocalIsolation {
    /// Per-run cgroup; `None` when cgroup setup failed or was skipped.
    /// Dropping this removes the cgroup.
    cgroup: Option<crate::jail::CgroupManager>,
    /// Pre-opened `cgroup.procs` file, inherited by the child so it can
    /// move itself into the cgroup before exec.
    procs: Option<std::fs::File>,
    /// Benchmark cores for the `sched_setaffinity` fallback.
    benchmark: Vec<usize>,
}

#[cfg(target_os = "linux")]
impl LocalIsolation {
    /// Prepare CPU isolation for a non-sandboxed run.
    ///
    /// Never fails: on any setup error (non-root, no cgroup v2) it warns
    /// and falls back to affinity-only isolation, or no isolation at all
    /// when the layout has none.
    pub(crate) fn prepare(layout: Option<&CpuLayout>) -> Self {
        let Some(layout) = layout.filter(|layout| layout.has_isolation()) else {
            return Self {
                cgroup: None,
                procs: None,
                benchmark: Vec::new(),
            };
        };

        let benchmark = layout.benchmark.clone();
        let run_id = format!("local-{}", uuid::Uuid::new_v4());

        let cgroup = match crate::jail::CgroupManager::new(&run_id) {
            Ok(cgroup) => {
                if let Err(e) = cgroup.apply_cpuset(layout) {
                    eprintln!("Warning: failed to apply cpuset for local run: {e}");
                }
                Some(cgroup)
            },
            Err(e) => {
                eprintln!(
                    "Warning: failed to create cgroup for local run (falling back to CPU affinity only): {e}"
                );
                None
            },
        };

        let procs = cgroup.as_ref().and_then(|cgroup| {
            match std::fs::OpenOptions::new()
                .write(true)
                .open(cgroup.path().join("cgroup.procs"))
            {
                Ok(file) => Some(file),
                Err(e) => {
                    eprintln!("Warning: failed to open cgroup.procs for local run: {e}");
                    None
                },
            }
        });

        if procs.is_some() {
            println!(
                "CPU isolation: benchmark pinned to cores {}",
                layout.benchmark_cpuset()
            );
        }

        Self {
            cgroup,
            procs,
            benchmark,
        }
    }

    /// Configure the benchmark command to attach itself to the cgroup (or
    /// pin its CPU affinity) in the child, between `fork` and `exec`.
    ///
    /// Attaching in the child instead of after `spawn` avoids the race
    /// where the benchmark runs startup work on the wrong cores.
    pub(crate) fn configure_command(&self, cmd: &mut Command) {
        use std::os::fd::AsRawFd;
        use std::os::unix::process::CommandExt as _;

        let procs_fd = self.procs.as_ref().map(AsRawFd::as_raw_fd);
        let benchmark = self.benchmark.clone();
        if procs_fd.is_none() && benchmark.is_empty() {
            return;
        }

        #[expect(
            unsafe_code,
            clippy::multiple_unsafe_ops_per_block,
            reason = "pre_exec and the async-signal-safe syscalls in it require unsafe"
        )]
        // SAFETY: The pre_exec closure runs in the forked child before exec,
        // so it may only use async-signal-safe operations. It performs no
        // allocation: `benchmark` was moved in before the fork, and the
        // closure only issues `write` and `sched_setaffinity` syscalls plus
        // stack memory manipulation (CPU_ZERO / CPU_SET). `procs_fd` stays
        // valid because `self.procs` outlives the spawn (the fd is inherited
        // across fork). All failures are ignored: isolation is best-effort
        // and must never prevent the benchmark from running.
        unsafe {
            cmd.pre_exec(move || {
                if let Some(fd) = procs_fd {
                    // Writing "0" to cgroup.procs moves the writing task.
                    _ = libc::write(fd, b"0".as_ptr().cast(), 1);
                }
                if !benchmark.is_empty() {
                    let mut set: libc::cpu_set_t = std::mem::zeroed();
                    libc::CPU_ZERO(&mut set);
                    for &cpu in &benchmark {
                        if cpu < libc::CPU_SETSIZE as usize {
                            libc::CPU_SET(cpu, &mut set);
                        }
                    }
                    _ = libc::sched_setaffinity(0, size_of::<libc::cpu_set_t>(), &raw const set);
                }
                Ok(())
            });
        }
    }

    /// Read cgroup resource metrics for this run, if a cgroup was created.
    ///
    /// Must be called before the isolation is dropped (dropping removes
    /// the cgroup).
    pub(crate) fn read_metrics(&self) -> Option<CgroupMetrics> {
        self.cgroup
            .as_ref()
            .and_then(|cgroup| crate::metrics::read_cgroup_metrics(cgroup.path()))
    }
}

// ---------------------------------------------------------------------------
// Non-Linux stub
// ---------------------------------------------------------------------------

/// No-op isolation for non-Linux platforms.
#[cfg(not(target_os = "linux"))]
pub(crate) struct LocalIsolation;

#[cfg(not(target_os = "linux"))]
#[expect(
    clippy::unused_self,
    reason = "non-Linux stub mirrors the Linux method signatures"
)]
impl LocalIsolation {
    pub(crate) fn prepare(_layout: Option<&CpuLayout>) -> Self {
        Self
    }

    pub(crate) fn configure_command(&self, _cmd: &mut Command) {}

    pub(crate) fn read_metrics(&self) -> Option<CgroupMetrics> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_layout_is_noop() {
        let isolation = LocalIsolation::prepare(None);
        assert!(isolation.read_metrics().is_none());
        #[cfg(target_os = "linux")]
        {
            assert!(isolation.cgroup.is_none());
            assert!(isolation.procs.is_none());
            assert!(isolation.benchmark.is_empty());
        }
    }

    #[test]
    fn layout_without_isolation_is_noop() {
        let layout = CpuLayout::with_core_count(1);
        let isolation = LocalIsolation::prepare(Some(&layout));
        assert!(isolation.read_metrics().is_none());
        #[cfg(target_os = "linux")]
        {
            assert!(isolation.cgroup.is_none());
            assert!(isolation.benchmark.is_empty());
        }
    }

    #[test]
    fn noop_isolation_leaves_command_spawnable() {
        let isolation = LocalIsolation::prepare(None);
        let mut cmd = Command::new("echo");
        isolation.configure_command(&mut cmd);
        let status = cmd
            .stdout(std::process::Stdio::null())
            .status()
            .expect("echo must spawn");
        assert!(status.success());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn configured_command_spawns_best_effort() {
        // Cgroup creation may succeed (root CI) or fall back (unprivileged);
        // either way the configured command must still spawn and succeed.
        let layout = CpuLayout::with_core_count(2);
        let isolation = LocalIsolation::prepare(Some(&layout));
        assert_eq!(isolation.benchmark, vec![1]);

        let mut cmd = Command::new("true");
        isolation.configure_command(&mut cmd);
        let status = cmd.status().expect("true must spawn");
        assert!(status.success());
    }
}
