//! Host system tuning for benchmark accuracy.
//!
//! Applies system-level optimizations (ASLR, CPU governor, SMT, etc.)
//! to reduce benchmark noise. All settings are restored on drop via
//! a RAII guard. Missing sysfs/procfs files are skipped with an
//! informational message - this handles ARM and other platforms
//! where certain controls do not exist.
//!
//! # Crash safety
//!
//! Restoration only happens on clean shutdown (guard drop). On SIGKILL
//! or panic-abort, sysctl writes, IRQ affinities, THP mode, and the
//! cpuset partition stay applied. Worse, the next run then reads the
//! already-tuned value as "current", saves nothing, and a later clean
//! exit cannot restore the true pre-tuning value; a reboot (or manual
//! reset) recovers it. Two mechanisms self-heal by construction: the
//! `/dev/cpu_dma_latency` fd releases its PM `QoS` constraint when the
//! process dies, and per-run cgroups are removed by their own cleanup.
//!
//! Tuning also assumes a single runner process per host: the sysctls,
//! IRQ affinities, THP mode, and cpuset partition are host-global, so a
//! second concurrent runner's shutdown restores them out from under the
//! first.

#![cfg_attr(
    target_os = "linux",
    expect(clippy::print_stdout, reason = "tuning prints applied settings")
)]

#[cfg(target_os = "linux")]
mod dma_latency;
#[cfg(target_os = "linux")]
mod kernel_work;
mod perf_event_paranoid;
pub mod preflight;
mod swappiness;
mod thp;

pub use perf_event_paranoid::PerfEventParanoid;
pub use swappiness::Swappiness;
pub use thp::{ParseThpModeError, ThpMode};

use crate::cpu::CpuLayout;

#[cfg(target_os = "linux")]
use camino::{Utf8Path, Utf8PathBuf};

#[cfg(target_os = "linux")]
const INTEL_NO_TURBO: &str = "/sys/devices/system/cpu/intel_pstate/no_turbo";
#[cfg(target_os = "linux")]
const CPUFREQ_BOOST: &str = "/sys/devices/system/cpu/cpufreq/boost";

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

#[cfg(target_os = "linux")]
struct SavedSetting {
    path: Utf8PathBuf,
    value: String,
    label: String,
}

/// RAII guard that restores host settings on drop.
#[cfg(target_os = "linux")]
pub struct TuningGuard {
    saved: Vec<SavedSetting>,
    /// File descriptors held open for the lifetime of the guard
    /// (e.g., the PM `QoS` constraint on `/dev/cpu_dma_latency`).
    /// Dropped after the saved settings are restored.
    held_fds: Vec<std::fs::File>,
}

#[cfg(target_os = "linux")]
impl TuningGuard {
    /// Record a file to restore to `value` when the guard drops.
    ///
    /// Restoration happens in reverse recording order, so dependent writes
    /// (e.g., a cpuset partition mode that must be reverted before its
    /// cpuset can shrink) are undone correctly.
    pub(crate) fn save_restore(&mut self, path: Utf8PathBuf, value: String, label: String) {
        self.saved.push(SavedSetting { path, value, label });
    }
}

#[cfg(target_os = "linux")]
impl Drop for TuningGuard {
    fn drop(&mut self) {
        for setting in self.saved.iter().rev() {
            restore(&setting.path, &setting.value, &setting.label);
        }
    }
}

/// Apply host tuning. Returns a guard that restores settings on drop.
#[cfg(target_os = "linux")]
pub fn apply(config: &TuningConfig) -> TuningGuard {
    let mut guard = TuningGuard {
        saved: Vec::new(),
        held_fds: Vec::new(),
    };

    if config.disable_aslr {
        write_sysctl(
            &mut guard,
            "/proc/sys/kernel/randomize_va_space",
            "0",
            "ASLR",
        );
    }

    if config.disable_nmi_watchdog {
        write_sysctl(
            &mut guard,
            "/proc/sys/kernel/nmi_watchdog",
            "0",
            "NMI watchdog",
        );
    }

    if let Some(val) = config.swappiness {
        write_sysctl(
            &mut guard,
            "/proc/sys/vm/swappiness",
            &val.to_string(),
            "swappiness",
        );
    }

    if let Some(val) = config.perf_event_paranoid {
        write_sysctl(
            &mut guard,
            "/proc/sys/kernel/perf_event_paranoid",
            &val.to_string(),
            "perf_event_paranoid",
        );
    }

    if let Some(gov) = &config.governor {
        set_cpu_governor(&mut guard, gov);
    }

    if config.disable_smt {
        set_smt(&mut guard);
    }

    if config.disable_turbo {
        set_turbo(&mut guard);
    }

    if config.disable_numa_balancing {
        write_sysctl(
            &mut guard,
            "/proc/sys/kernel/numa_balancing",
            "0",
            "NUMA balancing",
        );
    }

    if config.disable_timer_migration {
        write_sysctl(
            &mut guard,
            "/proc/sys/kernel/timer_migration",
            "0",
            "timer migration",
        );
    }

    if config.disable_soft_watchdog {
        write_sysctl(
            &mut guard,
            "/proc/sys/kernel/soft_watchdog",
            "0",
            "soft watchdog",
        );
    }

    if config.disable_ksm {
        write_sysctl(&mut guard, "/sys/kernel/mm/ksm/run", "0", "KSM");
    }

    if config.disable_cstates {
        dma_latency::hold_dma_latency(&mut guard, dma_latency::CPU_DMA_LATENCY);
    }

    if let Some(value) = config.thp.sysfs_value() {
        write_bracketed_sysctl(
            &mut guard,
            "/sys/kernel/mm/transparent_hugepage/enabled",
            value,
            "THP enabled",
        );
        write_bracketed_sysctl(
            &mut guard,
            "/sys/kernel/mm/transparent_hugepage/defrag",
            value,
            "THP defrag",
        );
    }

    guard
}

/// Apply CPU-layout-scoped tuning (IRQ and workqueue steering).
///
/// Must be called after [`apply`] and after [`CpuLayout::detect`], because
/// [`apply`] may disable SMT and change the core count. Settings are saved
/// on the same guard and restored on drop.
#[cfg(target_os = "linux")]
pub fn apply_cpu_scoped(config: &TuningConfig, layout: &CpuLayout, guard: &mut TuningGuard) {
    if config.cpuset_partition && layout.has_isolation() {
        let partition = crate::jail::BencherPartition::new(Utf8Path::new("/sys/fs/cgroup"));
        let level = partition.apply(layout, guard);
        println!("  Tuning: cpuset partition - achieved level '{level}'");
    }

    if config.steer_kernel_work && layout.has_isolation() {
        kernel_work::steer_kernel_work(guard, layout, Utf8Path::new("/"));
    }
}

/// Read current value, write new value, and push restore entry onto the guard.
#[cfg(target_os = "linux")]
fn write_sysctl(guard: &mut TuningGuard, path: &str, value: &str, label: &str) {
    let path = Utf8PathBuf::from(path);

    if !path.exists() {
        println!("  Tuning: {label} - skipped (path not found)");
        return;
    }

    let current = match std::fs::read_to_string(&path) {
        Ok(v) => v.trim().to_owned(),
        Err(e) => {
            println!("  Tuning: {label} - skipped (read failed: {e})");
            return;
        },
    };

    if current == value {
        println!("  Tuning: {label} - already {value}");
        return;
    }

    if let Err(e) = std::fs::write(path.as_str(), value) {
        println!("  Tuning: {label} - skipped (write failed: {e})");
        return;
    }

    println!("  Tuning: {label} - set to {value} (was {current})");
    guard.saved.push(SavedSetting {
        path,
        value: current,
        label: label.to_owned(),
    });
}

/// Like [`write_sysctl`], but for files using the bracketed selection
/// format (e.g., `always [madvise] never` under
/// `/sys/kernel/mm/transparent_hugepage/`). The current value is the
/// bracketed token; writes take the plain token, so save and restore
/// work with the parsed value.
#[cfg(target_os = "linux")]
fn write_bracketed_sysctl(guard: &mut TuningGuard, path: &str, value: &str, label: &str) {
    let path = Utf8PathBuf::from(path);

    if !path.exists() {
        println!("  Tuning: {label} - skipped (path not found)");
        return;
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(v) => v,
        Err(e) => {
            println!("  Tuning: {label} - skipped (read failed: {e})");
            return;
        },
    };
    let Some(current) = parse_bracketed_value(&content) else {
        println!(
            "  Tuning: {label} - skipped (unrecognized format: {})",
            content.trim()
        );
        return;
    };
    let current = current.to_owned();

    if current == value {
        println!("  Tuning: {label} - already {value}");
        return;
    }

    if let Err(e) = std::fs::write(path.as_str(), value) {
        println!("  Tuning: {label} - skipped (write failed: {e})");
        return;
    }

    println!("  Tuning: {label} - set to {value} (was {current})");
    guard.saved.push(SavedSetting {
        path,
        value: current,
        label: label.to_owned(),
    });
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
fn set_cpu_governor(guard: &mut TuningGuard, target: &str) {
    let base = Utf8Path::new("/sys/devices/system/cpu");
    let Ok(entries) = std::fs::read_dir(base) else {
        println!("  Tuning: CPU governor - skipped (cannot read {base})");
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

        if let Err(e) = std::fs::write(&gov_path, target) {
            println!("  Tuning: CPU governor ({name_str}) - skipped (write failed: {e})");
            continue;
        }

        println!("  Tuning: CPU governor ({name_str}) - set to {target} (was {current})");
        let gov_utf8_path = Utf8PathBuf::try_from(gov_path)
            .unwrap_or_else(|p| Utf8PathBuf::from(p.into_path_buf().to_string_lossy().as_ref()));
        guard.saved.push(SavedSetting {
            path: gov_utf8_path,
            value: current,
            label: format!("CPU governor ({name_str})"),
        });
    }
}

/// Disable SMT (simultaneous multi-threading / hyper-threading).
#[cfg(target_os = "linux")]
fn set_smt(guard: &mut TuningGuard) {
    let path = Utf8PathBuf::from("/sys/devices/system/cpu/smt/control");

    if !path.exists() {
        println!("  Tuning: SMT - skipped (not available on this platform)");
        return;
    }

    let current = match std::fs::read_to_string(&path) {
        Ok(v) => v.trim().to_owned(),
        Err(e) => {
            println!("  Tuning: SMT - skipped (read failed: {e})");
            return;
        },
    };

    if current == "off" || current == "notsupported" || current == "notimplemented" {
        println!("  Tuning: SMT - already {current}");
        return;
    }

    if let Err(e) = std::fs::write(path.as_str(), "off") {
        println!("  Tuning: SMT - skipped (write failed: {e})");
        return;
    }

    println!("  Tuning: SMT - disabled (was {current})");
    guard.saved.push(SavedSetting {
        path,
        value: current,
        label: "SMT".to_owned(),
    });
}

/// Disable turboboost. Tries Intel pstate first, then generic cpufreq.
#[cfg(target_os = "linux")]
fn set_turbo(guard: &mut TuningGuard) {
    if Utf8Path::new(INTEL_NO_TURBO).exists() {
        write_sysctl(guard, INTEL_NO_TURBO, "1", "turboboost (Intel)");
    } else if Utf8Path::new(CPUFREQ_BOOST).exists() {
        write_sysctl(guard, CPUFREQ_BOOST, "0", "turboboost (generic)");
    } else {
        println!("  Tuning: turboboost - skipped (not available on this platform)");
    }
}

/// Restore a single setting. Used by the Drop impl.
#[cfg(target_os = "linux")]
fn restore(path: &Utf8Path, value: &str, label: &str) {
    match std::fs::write(path.as_str(), value) {
        Ok(()) => println!("  Tuning: {label} - restored to {value}"),
        Err(e) => println!("  Tuning: {label} - restore failed: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Non-Linux stub
// ---------------------------------------------------------------------------

/// Stub guard for non-Linux platforms (no-op).
#[cfg(not(target_os = "linux"))]
pub struct TuningGuard;

/// No-op on non-Linux - returns a stub guard.
#[cfg(not(target_os = "linux"))]
pub fn apply(_config: &TuningConfig) -> TuningGuard {
    TuningGuard
}

/// No-op on non-Linux.
#[cfg(not(target_os = "linux"))]
pub fn apply_cpu_scoped(_config: &TuningConfig, _layout: &CpuLayout, _guard: &mut TuningGuard) {}

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
    fn apply_cpu_scoped_disabled_saves_nothing() {
        let config = TuningConfig::disabled();
        let layout = CpuLayout::with_core_count(8);
        let mut guard = apply(&config);
        apply_cpu_scoped(&config, &layout, &mut guard);
        #[cfg(target_os = "linux")]
        assert!(guard.saved.is_empty());
    }

    #[test]
    fn apply_cpu_scoped_no_isolation_saves_nothing() {
        let config = TuningConfig::default();
        let layout = CpuLayout::with_core_count(1);
        // Empty guard from a disabled config; the single-core layout means
        // apply_cpu_scoped must not touch anything.
        let mut guard = apply(&TuningConfig::disabled());
        apply_cpu_scoped(&config, &layout, &mut guard);
        #[cfg(target_os = "linux")]
        assert!(guard.saved.is_empty());
    }

    #[test]
    fn apply_returns_guard() {
        // On any platform, apply should return without panic
        let config = TuningConfig::default();
        let _guard = apply(&config);
    }

    #[test]
    fn apply_disabled_returns_guard() {
        let config = TuningConfig::disabled();
        let _guard = apply(&config);
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
    fn guard_drop_restores_via_tempfile() {
        use std::fs;

        let dir = tempfile::tempdir().unwrap();
        let path = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let file_path = path.join("test_sysctl");
        fs::write(&file_path, "original").unwrap();

        {
            let mut guard = TuningGuard {
                saved: Vec::new(),
                held_fds: Vec::new(),
            };
            guard.saved.push(SavedSetting {
                path: file_path.clone(),
                value: "original".to_owned(),
                label: "test".to_owned(),
            });
            // Simulate the tuning having changed the value
            fs::write(&file_path, "changed").unwrap();
            assert_eq!(fs::read_to_string(&file_path).unwrap(), "changed");
        }
        // Guard dropped - should restore
        assert_eq!(fs::read_to_string(&file_path).unwrap(), "original");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn guard_restores_in_reverse_order() {
        use std::fs;

        let dir = tempfile::tempdir().unwrap();
        let base = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let path1 = base.join("first");
        let path2 = base.join("second");
        fs::write(&path1, "a").unwrap();
        fs::write(&path2, "b").unwrap();

        {
            let mut guard = TuningGuard {
                saved: Vec::new(),
                held_fds: Vec::new(),
            };
            guard.saved.push(SavedSetting {
                path: path1.clone(),
                value: "a_orig".to_owned(),
                label: "first".to_owned(),
            });
            guard.saved.push(SavedSetting {
                path: path2.clone(),
                value: "b_orig".to_owned(),
                label: "second".to_owned(),
            });
        }
        // Both should be restored
        assert_eq!(fs::read_to_string(&path1).unwrap(), "a_orig");
        assert_eq!(fs::read_to_string(&path2).unwrap(), "b_orig");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn write_sysctl_skips_missing_path() {
        let mut guard = TuningGuard {
            saved: Vec::new(),
            held_fds: Vec::new(),
        };
        write_sysctl(&mut guard, "/nonexistent/path/value", "0", "test");
        assert!(
            guard.saved.is_empty(),
            "should not save anything for missing path"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn write_sysctl_skips_if_already_set() {
        use std::fs;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("value");
        fs::write(&path, "0").unwrap();

        let mut guard = TuningGuard {
            saved: Vec::new(),
            held_fds: Vec::new(),
        };
        write_sysctl(&mut guard, path.to_str().unwrap(), "0", "test");
        assert!(
            guard.saved.is_empty(),
            "should not save when value already matches"
        );
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
    fn write_bracketed_sysctl_saves_and_writes() {
        use std::fs;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("enabled");
        fs::write(&path, "always [madvise] never\n").unwrap();

        let mut guard = TuningGuard {
            saved: Vec::new(),
            held_fds: Vec::new(),
        };
        write_bracketed_sysctl(&mut guard, path.to_str().unwrap(), "never", "test");

        assert_eq!(guard.saved.len(), 1);
        assert_eq!(guard.saved[0].value, "madvise");
        assert_eq!(fs::read_to_string(&path).unwrap(), "never");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn write_bracketed_sysctl_skips_if_already_set() {
        use std::fs;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("enabled");
        fs::write(&path, "always madvise [never]\n").unwrap();

        let mut guard = TuningGuard {
            saved: Vec::new(),
            held_fds: Vec::new(),
        };
        write_bracketed_sysctl(&mut guard, path.to_str().unwrap(), "never", "test");

        assert!(guard.saved.is_empty());
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

        let mut guard = TuningGuard {
            saved: Vec::new(),
            held_fds: Vec::new(),
        };
        write_bracketed_sysctl(&mut guard, path.to_str().unwrap(), "never", "test");

        assert!(guard.saved.is_empty());
        assert_eq!(fs::read_to_string(&path).unwrap(), "garbage\n");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn write_sysctl_saves_and_writes() {
        use std::fs;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("value");
        fs::write(&path, "60").unwrap();

        let mut guard = TuningGuard {
            saved: Vec::new(),
            held_fds: Vec::new(),
        };
        write_sysctl(&mut guard, path.to_str().unwrap(), "10", "test");

        assert_eq!(guard.saved.len(), 1);
        assert_eq!(guard.saved[0].value, "60");
        assert_eq!(fs::read_to_string(&path).unwrap(), "10");
    }
}
