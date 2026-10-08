//! Host system tuning for benchmark accuracy.
//!
//! Applies system-level optimizations (ASLR, CPU governor, SMT, etc.)
//! to reduce benchmark noise. Missing sysfs/procfs files are skipped with an
//! informational message - this handles ARM and other platforms
//! where certain controls do not exist.
//!
//! Tuning is never reverted: every write is runtime-only and persists until
//! the host reboots. The one exception is the C-state hold, which the kernel
//! releases when the process exits.
//!
//! Tuning requires a single runner process per host: the sysctls, IRQ
//! affinities, THP mode, and cpuset partition are host-global, so a
//! second concurrent runner would rewrite them under the first's Jobs.
//! The runner lock enforces this: a runner takes it before [`apply`].

#[cfg(target_os = "linux")]
mod dma_latency;
#[cfg(target_os = "linux")]
mod kernel_work;
#[cfg(target_os = "linux")]
mod partition;
mod perf_event_paranoid;
pub mod preflight;
mod swappiness;
mod thp;

pub use perf_event_paranoid::PerfEventParanoid;
pub use swappiness::Swappiness;
pub use thp::{ParseThpModeError, ThpMode};

use slog::Logger;

use crate::cpu::CpuLayout;

#[cfg(target_os = "linux")]
use camino::{Utf8Path, Utf8PathBuf};
#[cfg(target_os = "linux")]
use slog::info;

#[cfg(target_os = "linux")]
const INTEL_NO_TURBO: &str = "/sys/devices/system/cpu/intel_pstate/no_turbo";
#[cfg(target_os = "linux")]
const CPUFREQ_BOOST: &str = "/sys/devices/system/cpu/cpufreq/boost";
#[cfg(target_os = "linux")]
const SMT_CONTROL: &str = "/sys/devices/system/cpu/smt/control";

/// Host tuning configuration - all defaults optimize for benchmark accuracy.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each bool maps to an independent system knob"
)]
#[derive(Debug, Clone)]
pub struct TuningConfig {
    /// Disable ASLR (default: true).
    pub disable_aslr: bool,
    /// Disable NMI watchdog (default: true).
    pub disable_nmi_watchdog: bool,
    /// Target swappiness value (default: Some(Swappiness(10))).
    pub swappiness: Option<Swappiness>,
    /// Target `perf_event_paranoid` value (default: Some(PerfEventParanoid(-1))).
    pub perf_event_paranoid: Option<PerfEventParanoid>,
    /// Target CPU scaling governor (default: Some("performance")).
    pub governor: Option<String>,
    /// Disable SMT / hyper-threading (default: true).
    pub disable_smt: bool,
    /// Disable turboboost (default: true).
    pub disable_turbo: bool,
    /// Disable automatic NUMA balancing page migration (default: true).
    pub disable_numa_balancing: bool,
    /// Disable timer migration between CPUs (default: true).
    pub disable_timer_migration: bool,
    /// Disable the soft lockup watchdog (default: true).
    pub disable_soft_watchdog: bool,
    /// Disable kernel samepage merging scanning (default: true).
    pub disable_ksm: bool,
    /// Hold CPUs out of deep C-states via `/dev/cpu_dma_latency` (default: true).
    pub disable_cstates: bool,
    /// Steer device IRQs and unbound workqueues to housekeeping cores
    /// (default: true; only applied when the CPU layout has isolation).
    pub steer_kernel_work: bool,
    /// Detach benchmark cores from the root scheduling domain via an
    /// isolated cpuset partition, the runtime equivalent of `isolcpus=`
    /// (default: true; only applied when the CPU layout has isolation).
    pub cpuset_partition: bool,
    /// Host transparent hugepage mode (default: `never` for deterministic
    /// memory backing; `leave` preserves the host configuration).
    pub thp: ThpMode,
}

impl Default for TuningConfig {
    fn default() -> Self {
        Self {
            disable_aslr: true,
            disable_nmi_watchdog: true,
            swappiness: Some(Swappiness::DEFAULT),
            perf_event_paranoid: Some(PerfEventParanoid::DEFAULT),
            governor: Some("performance".to_owned()),
            disable_smt: true,
            disable_turbo: true,
            disable_numa_balancing: true,
            disable_timer_migration: true,
            disable_soft_watchdog: true,
            disable_ksm: true,
            disable_cstates: true,
            steer_kernel_work: true,
            cpuset_partition: true,
            thp: ThpMode::Never,
        }
    }
}

impl TuningConfig {
    /// A config with all tuning disabled - no changes will be made.
    pub fn disabled() -> Self {
        Self {
            disable_aslr: false,
            disable_nmi_watchdog: false,
            swappiness: None,
            perf_event_paranoid: None,
            governor: None,
            disable_smt: false,
            disable_turbo: false,
            disable_numa_balancing: false,
            disable_timer_migration: false,
            disable_soft_watchdog: false,
            disable_ksm: false,
            disable_cstates: false,
            steer_kernel_work: false,
            cpuset_partition: false,
            thp: ThpMode::Leave,
        }
    }
}

// ---------------------------------------------------------------------------
// Linux implementation
// ---------------------------------------------------------------------------

/// Holds the C-state constraint, which the kernel releases when the process
/// exits.
#[cfg(target_os = "linux")]
#[must_use]
pub struct TuningGuard {
    held_fds: Vec<std::fs::File>,
}

/// Apply host tuning. Returns a guard that holds the C-state constraint.
#[cfg(target_os = "linux")]
pub fn apply(log: &Logger, config: &TuningConfig) -> TuningGuard {
    let mut guard = TuningGuard {
        held_fds: Vec::new(),
    };

    tune_sysctl(
        log,
        "/proc/sys/kernel/randomize_va_space",
        config.disable_aslr.then_some("0"),
        "ASLR",
    );

    tune_sysctl(
        log,
        "/proc/sys/kernel/nmi_watchdog",
        config.disable_nmi_watchdog.then_some("0"),
        "NMI watchdog",
    );

    let swappiness = config.swappiness.map(|val| val.to_string());
    tune_sysctl(
        log,
        "/proc/sys/vm/swappiness",
        swappiness.as_deref(),
        "swappiness",
    );

    let perf_event_paranoid = config.perf_event_paranoid.map(|val| val.to_string());
    tune_sysctl(
        log,
        "/proc/sys/kernel/perf_event_paranoid",
        perf_event_paranoid.as_deref(),
        "perf_event_paranoid",
    );

    if let Some(gov) = &config.governor {
        set_cpu_governor(log, gov);
    } else {
        log_left(
            log,
            "/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor",
            "CPU governor (cpu0)",
        );
    }

    if config.disable_smt {
        set_smt(log);
    } else {
        log_left(log, SMT_CONTROL, "SMT");
    }

    if config.disable_turbo {
        set_turbo(log);
    } else if Utf8Path::new(INTEL_NO_TURBO).exists() {
        log_left(log, INTEL_NO_TURBO, "turboboost (Intel)");
    } else {
        log_left(log, CPUFREQ_BOOST, "turboboost (generic)");
    }

    tune_sysctl(
        log,
        "/proc/sys/kernel/numa_balancing",
        config.disable_numa_balancing.then_some("0"),
        "NUMA balancing",
    );

    tune_sysctl(
        log,
        "/proc/sys/kernel/timer_migration",
        config.disable_timer_migration.then_some("0"),
        "timer migration",
    );

    tune_sysctl(
        log,
        "/proc/sys/kernel/soft_watchdog",
        config.disable_soft_watchdog.then_some("0"),
        "soft watchdog",
    );

    tune_sysctl(
        log,
        "/sys/kernel/mm/ksm/run",
        config.disable_ksm.then_some("0"),
        "KSM",
    );

    if config.disable_cstates {
        dma_latency::hold_dma_latency(log, &mut guard, Utf8Path::new(dma_latency::CPU_DMA_LATENCY));
    }

    let thp = config.thp.sysfs_value();
    tune_bracketed_sysctl(
        log,
        "/sys/kernel/mm/transparent_hugepage/enabled",
        thp,
        "THP enabled",
    );
    tune_bracketed_sysctl(
        log,
        "/sys/kernel/mm/transparent_hugepage/defrag",
        thp,
        "THP defrag",
    );

    guard
}

/// Apply CPU-layout-scoped tuning (IRQ and workqueue steering).
///
/// Must be called after [`apply`] and after [`CpuLayout::detect`], because
/// [`apply`] may disable SMT and change the core count.
#[cfg(target_os = "linux")]
pub fn apply_cpu_scoped(log: &Logger, config: &TuningConfig, layout: &CpuLayout) {
    apply_cpu_scoped_at(log, config, layout, Utf8Path::new("/"));
}

/// Like [`apply_cpu_scoped`], under the filesystem root `root`.
#[cfg(target_os = "linux")]
fn apply_cpu_scoped_at(log: &Logger, config: &TuningConfig, layout: &CpuLayout, root: &Utf8Path) {
    let bencher = partition::BencherPartition::new(&root.join("sys/fs/cgroup"));
    if config.cpuset_partition && layout.has_isolation() {
        let level = bencher.apply(log, layout);
        info!(log, "Tuning";
            "setting" => "cpuset partition",
            "action" => "achieved",
            "value" => %level,
        );
    } else if !config.cpuset_partition {
        log_left(log, bencher.partition_path().as_str(), "cpuset partition");
    }

    if config.steer_kernel_work && layout.has_isolation() {
        kernel_work::steer_kernel_work(log, layout, root);
    } else if !config.steer_kernel_work {
        kernel_work::log_steering(log, root);
    }
}

/// Write `target` unless it already holds, or log the current value when
/// the knob is skipped.
#[cfg(target_os = "linux")]
fn tune_sysctl(log: &Logger, path: &str, target: Option<&str>, label: &str) {
    if let Some(target) = target {
        write_sysctl(log, path, target, label);
    } else {
        log_left(log, path, label);
    }
}

/// Like [`tune_sysctl`], for [`write_bracketed_sysctl`].
#[cfg(target_os = "linux")]
fn tune_bracketed_sysctl(log: &Logger, path: &str, target: Option<&str>, label: &str) {
    if let Some(target) = target {
        write_bracketed_sysctl(log, path, target, label);
    } else {
        log_left(log, path, label);
    }
}

/// Log a skipped knob's current value, since tuning from an earlier run
/// stays until the host reboots.
#[cfg(target_os = "linux")]
fn log_left(log: &Logger, path: &str, label: &str) {
    if let Ok(current) = std::fs::read_to_string(path) {
        info!(log, "Tuning"; "setting" => label, "action" => "left", "value" => current.trim());
    }
}

/// Read the current value and write the new one unless it already holds.
#[cfg(target_os = "linux")]
fn write_sysctl(log: &Logger, path: &str, value: &str, label: &str) {
    write_sysctl_with(log, path, value, label, |current, value| current == value);
}

/// Like [`write_sysctl`], with `same` deciding whether the current value
/// already holds.
#[cfg(target_os = "linux")]
fn write_sysctl_with(
    log: &Logger,
    path: &str,
    value: &str,
    label: &str,
    same: fn(&str, &str) -> bool,
) {
    let path = Utf8PathBuf::from(path);

    if !path.exists() {
        log_skipped(log, label, "path not found");
        return;
    }

    let current = match std::fs::read_to_string(&path) {
        Ok(v) => v.trim().to_owned(),
        Err(e) => {
            log_failed(log, label, "read failed", &e);
            return;
        },
    };

    if same(&current, value) {
        info!(log, "Tuning"; "setting" => label, "action" => "already", "value" => value);
        return;
    }

    if let Err(e) = std::fs::write(path.as_str(), value) {
        log_failed(log, label, "write failed", &e);
        return;
    }

    info!(log, "Tuning"; "setting" => label, "action" => "set", "value" => value, "was" => current);
}

/// Like [`write_sysctl`], but for files using the bracketed selection
/// format (e.g., `always [madvise] never` under
/// `/sys/kernel/mm/transparent_hugepage/`). The current value is the
/// bracketed token; writes take the plain token.
#[cfg(target_os = "linux")]
fn write_bracketed_sysctl(log: &Logger, path: &str, value: &str, label: &str) {
    let path = Utf8PathBuf::from(path);

    if !path.exists() {
        log_skipped(log, label, "path not found");
        return;
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(v) => v,
        Err(e) => {
            log_failed(log, label, "read failed", &e);
            return;
        },
    };
    let Some(current) = parse_bracketed_value(&content) else {
        info!(log, "Tuning";
            "setting" => label,
            "action" => "skipped",
            "reason" => "unrecognized format",
            "value" => bencher_logger::capped(content.trim()),
        );
        return;
    };
    let current = current.to_owned();

    if current == value {
        info!(log, "Tuning"; "setting" => label, "action" => "already", "value" => value);
        return;
    }

    if let Err(e) = std::fs::write(path.as_str(), value) {
        log_failed(log, label, "write failed", &e);
        return;
    }

    info!(log, "Tuning"; "setting" => label, "action" => "set", "value" => value, "was" => current);
}

/// Extract the selected token from a bracketed sysfs value like
/// `always [madvise] never`.
#[cfg(target_os = "linux")]
fn parse_bracketed_value(content: &str) -> Option<&str> {
    content
        .split_whitespace()
        .find_map(|token| token.strip_prefix('[')?.strip_suffix(']'))
}

/// Set the CPU scaling governor on all CPUs.
#[cfg(target_os = "linux")]
fn set_cpu_governor(log: &Logger, target: &str) {
    let base = Utf8Path::new("/sys/devices/system/cpu");
    let Ok(entries) = std::fs::read_dir(base) else {
        log_skipped(log, "CPU governor", "cpu directory unreadable");
        return;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if !name_str.starts_with("cpu")
            || !name_str
                .get(3..)
                .and_then(|s| s.chars().next())
                .is_some_and(|c| c.is_ascii_digit())
        {
            continue;
        }

        let gov_path = entry.path().join("cpufreq/scaling_governor");
        if !gov_path.exists() {
            continue;
        }

        let current = match std::fs::read_to_string(&gov_path) {
            Ok(v) => v.trim().to_owned(),
            Err(_) => continue,
        };

        if current == target {
            continue;
        }

        let setting = format!("CPU governor ({name_str})");
        if let Err(e) = std::fs::write(&gov_path, target) {
            log_failed(log, &setting, "write failed", &e);
            continue;
        }

        info!(log, "Tuning"; "setting" => setting, "action" => "set", "value" => target, "was" => current);
    }
}

/// Disable SMT (simultaneous multi-threading / hyper-threading).
#[cfg(target_os = "linux")]
fn set_smt(log: &Logger) {
    let path = Utf8PathBuf::from(SMT_CONTROL);

    if !path.exists() {
        log_skipped(log, "SMT", "not available on this platform");
        return;
    }

    let current = match std::fs::read_to_string(&path) {
        Ok(v) => v.trim().to_owned(),
        Err(e) => {
            log_failed(log, "SMT", "read failed", &e);
            return;
        },
    };

    if current == "off" || current == "notsupported" || current == "notimplemented" {
        info!(log, "Tuning"; "setting" => "SMT", "action" => "already", "value" => current);
        return;
    }

    if let Err(e) = std::fs::write(path.as_str(), "off") {
        log_failed(log, "SMT", "write failed", &e);
        return;
    }

    info!(log, "Tuning"; "setting" => "SMT", "action" => "set", "value" => "off", "was" => current);
}

/// Disable turboboost. Tries Intel pstate first, then generic cpufreq.
#[cfg(target_os = "linux")]
fn set_turbo(log: &Logger) {
    if Utf8Path::new(INTEL_NO_TURBO).exists() {
        write_sysctl(log, INTEL_NO_TURBO, "1", "turboboost (Intel)");
    } else if Utf8Path::new(CPUFREQ_BOOST).exists() {
        write_sysctl(log, CPUFREQ_BOOST, "0", "turboboost (generic)");
    } else {
        log_skipped(log, "turboboost", "not available on this platform");
    }
}

#[cfg(target_os = "linux")]
fn log_skipped(log: &Logger, setting: &str, reason: &str) {
    info!(log, "Tuning"; "setting" => setting, "action" => "skipped", "reason" => reason);
}

#[cfg(target_os = "linux")]
fn log_failed(log: &Logger, setting: &str, reason: &str, error: &std::io::Error) {
    info!(log, "Tuning";
        "setting" => setting,
        "action" => "skipped",
        "reason" => reason,
        "error" => %error,
    );
}

// ---------------------------------------------------------------------------
// Non-Linux stub
// ---------------------------------------------------------------------------

/// Stub guard for non-Linux platforms (no-op).
#[cfg(not(target_os = "linux"))]
#[must_use]
pub struct TuningGuard;

/// No-op on non-Linux - returns a stub guard.
#[cfg(not(target_os = "linux"))]
pub fn apply(_log: &Logger, _config: &TuningConfig) -> TuningGuard {
    TuningGuard
}

/// No-op on non-Linux.
#[cfg(not(target_os = "linux"))]
pub fn apply_cpu_scoped(_log: &Logger, _config: &TuningConfig, _layout: &CpuLayout) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_enables_all_tuning() {
        let config = TuningConfig::default();
        assert!(config.disable_aslr);
        assert!(config.disable_nmi_watchdog);
        assert_eq!(config.swappiness, Some(Swappiness::try_from(10).unwrap()));
        assert_eq!(
            config.perf_event_paranoid,
            Some(PerfEventParanoid::try_from(-1).unwrap())
        );
        assert_eq!(config.governor.as_deref(), Some("performance"));
        assert!(config.disable_smt);
        assert!(config.disable_turbo);
        assert!(config.disable_numa_balancing);
        assert!(config.disable_timer_migration);
        assert!(config.disable_soft_watchdog);
        assert!(config.disable_ksm);
        assert!(config.disable_cstates);
        assert!(config.steer_kernel_work);
        assert!(config.cpuset_partition);
        assert_eq!(config.thp, ThpMode::Never);
    }

    #[test]
    fn disabled_config_changes_nothing() {
        let config = TuningConfig::disabled();
        assert!(!config.disable_aslr);
        assert!(!config.disable_nmi_watchdog);
        assert_eq!(config.swappiness, None);
        assert_eq!(config.perf_event_paranoid, None);
        assert_eq!(config.governor, None);
        assert!(!config.disable_smt);
        assert!(!config.disable_turbo);
        assert!(!config.disable_numa_balancing);
        assert!(!config.disable_timer_migration);
        assert!(!config.disable_soft_watchdog);
        assert!(!config.disable_ksm);
        assert!(!config.disable_cstates);
        assert!(!config.steer_kernel_work);
        assert!(!config.cpuset_partition);
        assert_eq!(config.thp, ThpMode::Leave);
    }

    #[test]
    fn apply_disabled_returns_guard() {
        let config = TuningConfig::disabled();
        let _guard = apply(&crate::log::discard(), &config);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_opt_outs_leave_the_partition_and_the_steering_masks_alone() {
        // Kills dropping `--no-cpuset-partition` or `--no-irq-steering` from its gate.
        let (_dir, root) = fake_host_root();
        let before = tree(&root);
        let config = TuningConfig {
            cpuset_partition: false,
            steer_kernel_work: false,
            ..TuningConfig::default()
        };

        apply_cpu_scoped_at(
            &crate::log::discard(),
            &config,
            &CpuLayout::with_core_count(8),
            &root,
        );

        assert_eq!(tree(&root), before);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_single_cpu_skips_cpu_scoped_tuning() {
        // Kills dropping `has_isolation` from either gate.
        let (_dir, root) = fake_host_root();
        let before = tree(&root);

        apply_cpu_scoped_at(
            &crate::log::discard(),
            &TuningConfig::default(),
            &CpuLayout::with_core_count(1),
            &root,
        );

        assert_eq!(tree(&root), before);
    }

    #[test]
    fn config_clone() {
        let config = TuningConfig::default();
        let cloned = config.clone();
        assert_eq!(config.disable_aslr, cloned.disable_aslr);
        assert_eq!(config.swappiness, cloned.swappiness);
        assert_eq!(config.governor, cloned.governor);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_restart_leaves_an_already_set_value_unwritten() {
        // Kills dropping the equality check: the kernel prints a newline the
        // runner never writes, so a rewrite loses it.
        use std::fs;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("value");
        fs::write(&path, "0\n").unwrap();

        write_sysctl(&crate::log::discard(), path.to_str().unwrap(), "0", "test");

        assert_eq!(fs::read_to_string(&path).unwrap(), "0\n");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn parse_bracketed_value_variants() {
        assert_eq!(
            parse_bracketed_value("always [madvise] never\n"),
            Some("madvise")
        );
        assert_eq!(
            parse_bracketed_value("[always] madvise never"),
            Some("always")
        );
        assert_eq!(
            parse_bracketed_value("always defer defer+madvise madvise [never]\n"),
            Some("never")
        );
        assert_eq!(parse_bracketed_value("no brackets here"), None);
        assert_eq!(parse_bracketed_value(""), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn write_bracketed_sysctl_writes_the_plain_token() {
        use std::fs;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("enabled");
        fs::write(&path, "always [madvise] never\n").unwrap();

        write_bracketed_sysctl(
            &crate::log::discard(),
            path.to_str().unwrap(),
            "never",
            "test",
        );

        assert_eq!(fs::read_to_string(&path).unwrap(), "never");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn write_bracketed_sysctl_skips_if_already_set() {
        use std::fs;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("enabled");
        fs::write(&path, "always madvise [never]\n").unwrap();

        write_bracketed_sysctl(
            &crate::log::discard(),
            path.to_str().unwrap(),
            "never",
            "test",
        );

        // The file is untouched (still in the kernel's bracketed format)
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "always madvise [never]\n"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn write_bracketed_sysctl_skips_unrecognized_format() {
        use std::fs;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("enabled");
        fs::write(&path, "garbage\n").unwrap();

        write_bracketed_sysctl(
            &crate::log::discard(),
            path.to_str().unwrap(),
            "never",
            "test",
        );

        assert_eq!(fs::read_to_string(&path).unwrap(), "garbage\n");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn write_sysctl_writes_the_target() {
        use std::fs;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("value");
        fs::write(&path, "60").unwrap();

        write_sysctl(&crate::log::discard(), path.to_str().unwrap(), "10", "test");

        assert_eq!(fs::read_to_string(&path).unwrap(), "10");
    }

    /// A fake untuned host holding the files CPU-scoped tuning writes.
    #[cfg(target_os = "linux")]
    fn fake_host_root() -> (tempfile::TempDir, Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        for (path, contents) in [
            ("sys/fs/cgroup/cgroup.controllers", "cpuset memory pids\n"),
            ("sys/fs/cgroup/cgroup.subtree_control", "\n"),
            ("proc/irq/default_smp_affinity", "ff\n"),
            ("proc/irq/10/smp_affinity_list", "0-7\n"),
            ("sys/devices/virtual/workqueue/cpumask", "ff\n"),
        ] {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }

        (dir, root)
    }

    /// Every path under `root` with its contents, `None` for a directory.
    #[cfg(target_os = "linux")]
    fn tree(root: &Utf8Path) -> std::collections::BTreeMap<Utf8PathBuf, Option<String>> {
        let mut tree = std::collections::BTreeMap::new();
        let mut dirs = vec![root.to_owned()];
        while let Some(dir) = dirs.pop() {
            for entry in dir.read_dir_utf8().unwrap() {
                let path = entry.unwrap().into_path();
                if path.is_dir() {
                    tree.insert(path.clone(), None);
                    dirs.push(path);
                } else {
                    let contents = std::fs::read_to_string(&path).unwrap();
                    tree.insert(path, Some(contents));
                }
            }
        }
        tree
    }
}
