//! Host system tuning for benchmark accuracy.
//!
//! Applies system-level optimizations (ASLR, CPU governor, SMT, etc.)
//! to reduce benchmark noise. All settings are restored on drop via
//! a RAII guard. Missing sysfs/procfs files are skipped with an
//! informational message — this handles ARM and other platforms
//! where certain controls do not exist.

#[cfg(target_os = "linux")]
use std::path::PathBuf;

/// Host tuning configuration — all defaults optimize for benchmark accuracy.
#[expect(clippy::struct_excessive_bools, reason = "each bool maps to an independent system knob")]
#[derive(Debug, Clone)]
pub struct TuningConfig {
    /// Disable ASLR (default: true).
    pub disable_aslr: bool,
    /// Disable NMI watchdog (default: true).
    pub disable_nmi_watchdog: bool,
    /// Target swappiness value (default: Some(10)).
    pub swappiness: Option<u32>,
    /// Target `perf_event_paranoid` value (default: Some(-1)).
    pub perf_event_paranoid: Option<i32>,
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
            swappiness: Some(10),
            perf_event_paranoid: Some(-1),
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
    path: PathBuf,
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

    if let Some(ref gov) = config.governor {
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
    let path = PathBuf::from(path);

    if !path.exists() {
        println!("  Tuning: {label} — skipped (path not found)");
        return;
    }

    let current = match std::fs::read_to_string(&path) {
        Ok(v) => v.trim().to_owned(),
        Err(e) => {
            println!("  Tuning: {label} — skipped (read failed: {e})");
            return;
        }
    };

    if current == value {
        println!("  Tuning: {label} — already {value}");
        return;
    }

    if let Err(e) = std::fs::write(&path, value) {
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
    let base = PathBuf::from("/sys/devices/system/cpu");
    let Ok(entries) = std::fs::read_dir(&base) else {
        println!("  Tuning: CPU governor — skipped (cannot read {base:?})");
        return;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if !name_str.starts_with("cpu") || !name_str[3..].chars().next().is_some_and(|c| c.is_ascii_digit()) {
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
        guard.saved.push(SavedSetting {
            path: gov_path,
            value: current,
            label: format!("CPU governor ({name_str})"),
        });
    }
}

/// Disable SMT (simultaneous multi-threading / hyper-threading).
#[cfg(target_os = "linux")]
fn set_smt(guard: &mut TuningGuard) {
    let path = PathBuf::from("/sys/devices/system/cpu/smt/control");

    if !path.exists() {
        println!("  Tuning: SMT — skipped (not available on this platform)");
        return;
    }

    let current = match std::fs::read_to_string(&path) {
        Ok(v) => v.trim().to_owned(),
        Err(e) => {
            println!("  Tuning: SMT — skipped (read failed: {e})");
            return;
        }
    };

    if current == "off" || current == "notsupported" || current == "notimplemented" {
        println!("  Tuning: SMT — already {current}");
        return;
    }

    if let Err(e) = std::fs::write(&path, "off") {
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
    let intel_path = PathBuf::from("/sys/devices/system/cpu/intel_pstate/no_turbo");
    let generic_path = PathBuf::from("/sys/devices/system/cpu/cpufreq/boost");

    if intel_path.exists() {
        // Intel: write "1" to no_turbo to disable turbo
        write_sysctl(guard, intel_path.to_str().unwrap_or_default(), "1", "turboboost (Intel)");
    } else if generic_path.exists() {
        // Generic: write "0" to boost to disable turbo
        write_sysctl(guard, generic_path.to_str().unwrap_or_default(), "0", "turboboost (generic)");
    } else {
        println!("  Tuning: turboboost — skipped (not available on this platform)");
    }
}

/// Restore a single setting. Used by the Drop impl.
#[cfg(target_os = "linux")]
fn restore(path: &PathBuf, value: &str, label: &str) {
    match std::fs::write(path, value) {
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
