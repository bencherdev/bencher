//! Host system tuning for benchmark accuracy.
//!
//! Applies system-level optimizations (ASLR, CPU governor, SMT, etc.)
//! to reduce benchmark noise. All settings are restored on drop via
//! a RAII guard. Missing sysfs/procfs files are skipped with an
//! informational message — this handles ARM and other platforms
//! where certain controls do not exist.

#![cfg_attr(target_os = "linux", expect(clippy::print_stdout))]

mod perf_event_paranoid;
mod swappiness;

pub use perf_event_paranoid::PerfEventParanoid;
pub use swappiness::Swappiness;

#[cfg(target_os = "linux")]
use camino::{Utf8Path, Utf8PathBuf};

#[cfg(target_os = "linux")]
const INTEL_NO_TURBO: &str = "/sys/devices/system/cpu/intel_pstate/no_turbo";
#[cfg(target_os = "linux")]
const CPUFREQ_BOOST: &str = "/sys/devices/system/cpu/cpufreq/boost";

/// Host tuning configuration — all defaults optimize for benchmark accuracy.
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
        }
    }
}

impl TuningConfig {
    /// A config with all tuning disabled — no changes will be made.
    pub fn disabled() -> Self {
        Self {
            disable_aslr: false,
            disable_nmi_watchdog: false,
            swappiness: None,
            perf_event_paranoid: None,
            governor: None,
            disable_smt: false,
            disable_turbo: false,
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
    let mut guard = TuningGuard { saved: Vec::new() };

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

    guard
}

/// Read current value, write new value, and push restore entry onto the guard.
#[cfg(target_os = "linux")]
fn write_sysctl(guard: &mut TuningGuard, path: &str, value: &str, label: &str) {
    let path = Utf8PathBuf::from(path);

    if !path.exists() {
        println!("  Tuning: {label} — skipped (path not found)");
        return;
    }

    let current = match std::fs::read_to_string(&path) {
        Ok(v) => v.trim().to_owned(),
        Err(e) => {
            println!("  Tuning: {label} — skipped (read failed: {e})");
            return;
        },
    };

    if current == value {
        println!("  Tuning: {label} — already {value}");
        return;
    }

    if let Err(e) = std::fs::write(path.as_str(), value) {
        println!("  Tuning: {label} — skipped (write failed: {e})");
        return;
    }

    println!("  Tuning: {label} — set to {value} (was {current})");
    guard.saved.push(SavedSetting {
        path,
        value: current,
        label: label.to_owned(),
    });
}

/// Set the CPU scaling governor on all CPUs.
#[cfg(target_os = "linux")]
fn set_cpu_governor(guard: &mut TuningGuard, target: &str) {
    let base = Utf8Path::new("/sys/devices/system/cpu");
    let Ok(entries) = std::fs::read_dir(base) else {
        println!("  Tuning: CPU governor — skipped (cannot read {base})");
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
            println!("  Tuning: CPU governor ({name_str}) — skipped (write failed: {e})");
            continue;
        }

        println!("  Tuning: CPU governor ({name_str}) — set to {target} (was {current})");
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
        println!("  Tuning: SMT — skipped (not available on this platform)");
        return;
    }

    let current = match std::fs::read_to_string(&path) {
        Ok(v) => v.trim().to_owned(),
        Err(e) => {
            println!("  Tuning: SMT — skipped (read failed: {e})");
            return;
        },
    };

    if current == "off" || current == "notsupported" || current == "notimplemented" {
        println!("  Tuning: SMT — already {current}");
        return;
    }

    if let Err(e) = std::fs::write(path.as_str(), "off") {
        println!("  Tuning: SMT — skipped (write failed: {e})");
        return;
    }

    println!("  Tuning: SMT — disabled (was {current})");
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
        println!("  Tuning: turboboost — skipped (not available on this platform)");
    }
}

/// Restore a single setting. Used by the Drop impl.
#[cfg(target_os = "linux")]
fn restore(path: &Utf8Path, value: &str, label: &str) {
    match std::fs::write(path.as_str(), value) {
        Ok(()) => println!("  Tuning: {label} — restored to {value}"),
        Err(e) => println!("  Tuning: {label} — restore failed: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Non-Linux stub
// ---------------------------------------------------------------------------

/// Stub guard for non-Linux platforms (no-op).
#[cfg(not(target_os = "linux"))]
pub struct TuningGuard;

/// No-op on non-Linux — returns a stub guard.
#[cfg(not(target_os = "linux"))]
pub fn apply(_config: &TuningConfig) -> TuningGuard {
    TuningGuard
}

#[cfg(test)]
#[cfg_attr(target_os = "linux", expect(clippy::indexing_slicing))]
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
            let mut guard = TuningGuard { saved: Vec::new() };
            guard.saved.push(SavedSetting {
                path: file_path.clone(),
                value: "original".to_owned(),
                label: "test".to_owned(),
            });
            // Simulate the tuning having changed the value
            fs::write(&file_path, "changed").unwrap();
            assert_eq!(fs::read_to_string(&file_path).unwrap(), "changed");
        }
        // Guard dropped — should restore
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
            let mut guard = TuningGuard { saved: Vec::new() };
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
        let mut guard = TuningGuard { saved: Vec::new() };
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

        let mut guard = TuningGuard { saved: Vec::new() };
        write_sysctl(&mut guard, path.to_str().unwrap(), "0", "test");
        assert!(
            guard.saved.is_empty(),
            "should not save when value already matches"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn write_sysctl_saves_and_writes() {
        use std::fs;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("value");
        fs::write(&path, "60").unwrap();

        let mut guard = TuningGuard { saved: Vec::new() };
        write_sysctl(&mut guard, path.to_str().unwrap(), "10", "test");

        assert_eq!(guard.saved.len(), 1);
        assert_eq!(guard.saved[0].value, "60");
        assert_eq!(fs::read_to_string(&path).unwrap(), "10");
    }
}
