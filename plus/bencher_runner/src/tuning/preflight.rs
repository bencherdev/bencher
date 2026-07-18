//! Static preflight checks for host conditions that limit benchmark accuracy.
//!
//! These checks detect conditions the runner cannot fix at runtime
//! (virtualization, missing cgroup v2) or that would fight the runner's
//! own tuning (irqbalance). They are informational only: the runner
//! proceeds either way, falling back to its runtime isolation measures.

use camino::Utf8Path;

/// A host condition that may increase benchmark variance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreflightWarning {
    /// The irqbalance daemon is running and will fight IRQ steering.
    IrqBalanceRunning,
    /// cgroup v2 is not available, so cpuset-based CPU isolation is not possible.
    NoCgroupV2,
    /// The host appears to be virtualized; hypervisor steal time adds variance.
    VirtualizedHost,
    /// The kernel booted without CPU isolation boot args; the runtime cpuset
    /// partition is used as a fallback.
    NoIsolationBootArgs,
}

impl std::fmt::Display for PreflightWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::IrqBalanceRunning => write!(
                f,
                "irqbalance is running and will fight IRQ steering; consider `systemctl disable --now irqbalance`"
            ),
            Self::NoCgroupV2 => write!(
                f,
                "cgroup v2 is not available; cpuset-based CPU isolation will be skipped"
            ),
            Self::VirtualizedHost => write!(
                f,
                "host appears to be virtualized; hypervisor steal time adds variance, prefer bare metal"
            ),
            Self::NoIsolationBootArgs => write!(
                f,
                "kernel booted without `isolcpus=`/`nohz_full=`; for full isolation add boot args `isolcpus= nohz_full= rcu_nocbs=` listing the benchmark cores (when tuning is enabled, the runtime cpuset partition compensates in part)"
            ),
        }
    }
}

/// Detect preflight warnings under the given filesystem root.
///
/// `root` is `/` in production; tests pass a tempdir tree containing
/// `proc/` and `sys/` subtrees.
#[must_use]
pub fn detect(root: &Utf8Path) -> Vec<PreflightWarning> {
    let mut warnings = Vec::new();

    if irqbalance_running(root) {
        warnings.push(PreflightWarning::IrqBalanceRunning);
    }

    if !root.join("sys/fs/cgroup/cgroup.controllers").exists() {
        warnings.push(PreflightWarning::NoCgroupV2);
    }

    if virtualized_host(root) {
        warnings.push(PreflightWarning::VirtualizedHost);
    }

    // Only warn when the cmdline is readable and lacks the isolation args;
    // an unreadable cmdline gives no signal either way.
    if let Ok(cmdline) = std::fs::read_to_string(root.join("proc/cmdline").as_std_path())
        && !cmdline.contains("isolcpus=")
        && !cmdline.contains("nohz_full=")
    {
        warnings.push(PreflightWarning::NoIsolationBootArgs);
    }

    warnings
}

/// Detect and print preflight warnings for the live host (Linux only).
pub fn print_host_warnings() {
    #[cfg(target_os = "linux")]
    for warning in detect(Utf8Path::new("/")) {
        println!("  Preflight: {warning}");
    }
}

/// Check whether an irqbalance process is running by scanning `proc/*/comm`.
fn irqbalance_running(root: &Utf8Path) -> bool {
    let Ok(entries) = std::fs::read_dir(root.join("proc").as_std_path()) else {
        return false;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if name_str.is_empty() || !name_str.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        if let Ok(comm) = std::fs::read_to_string(entry.path().join("comm"))
            && comm.trim() == "irqbalance"
        {
            return true;
        }
    }

    false
}

/// Hypervisor strings found in DMI system vendor or product name.
const HYPERVISOR_MARKERS: &[&str] = &["QEMU", "KVM", "VMware", "Xen", "innotek", "Virtual Machine"];

/// Heuristic virtualization detection.
///
/// Checks the x86 `hypervisor` cpuinfo flag and, as a cross-arch fallback,
/// well-known hypervisor strings in the DMI system vendor and product name.
fn virtualized_host(root: &Utf8Path) -> bool {
    if let Ok(cpuinfo) = std::fs::read_to_string(root.join("proc/cpuinfo").as_std_path())
        && cpuinfo.lines().any(|line| {
            line.starts_with("flags") && line.split_whitespace().any(|flag| flag == "hypervisor")
        })
    {
        return true;
    }

    for dmi_file in ["sys_vendor", "product_name"] {
        if let Ok(value) = std::fs::read_to_string(
            root.join("sys/devices/virtual/dmi/id")
                .join(dmi_file)
                .as_std_path(),
        ) && HYPERVISOR_MARKERS
            .iter()
            .any(|marker| value.contains(marker))
        {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use std::fs;

    use camino::Utf8PathBuf;

    use super::*;

    /// A root that raises none of the warnings.
    fn quiet_root() -> (tempfile::TempDir, Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        fs::create_dir_all(root.join("proc")).unwrap();
        fs::create_dir_all(root.join("sys/fs/cgroup")).unwrap();
        fs::write(root.join("sys/fs/cgroup/cgroup.controllers"), "cpuset cpu").unwrap();
        fs::write(root.join("proc/cmdline"), "isolcpus=2-7 nohz_full=2-7\n").unwrap();

        (dir, root)
    }

    #[test]
    fn quiet_host_no_warnings() {
        let (_dir, root) = quiet_root();
        assert_eq!(detect(&root), Vec::new());
    }

    #[test]
    fn detects_irqbalance() {
        let (_dir, root) = quiet_root();
        fs::create_dir_all(root.join("proc/123")).unwrap();
        fs::write(root.join("proc/123/comm"), "irqbalance\n").unwrap();

        assert!(detect(&root).contains(&PreflightWarning::IrqBalanceRunning));
    }

    #[test]
    fn other_processes_not_flagged() {
        let (_dir, root) = quiet_root();
        fs::create_dir_all(root.join("proc/123")).unwrap();
        fs::write(root.join("proc/123/comm"), "bash\n").unwrap();

        assert!(!detect(&root).contains(&PreflightWarning::IrqBalanceRunning));
    }

    #[test]
    fn detects_missing_cgroup_v2() {
        let (_dir, root) = quiet_root();
        fs::remove_file(root.join("sys/fs/cgroup/cgroup.controllers")).unwrap();

        assert!(detect(&root).contains(&PreflightWarning::NoCgroupV2));
    }

    #[test]
    fn detects_hypervisor_cpuinfo_flag() {
        let (_dir, root) = quiet_root();
        fs::write(
            root.join("proc/cpuinfo"),
            "processor : 0\nflags : fpu vme hypervisor sse2\n",
        )
        .unwrap();

        assert!(detect(&root).contains(&PreflightWarning::VirtualizedHost));
    }

    #[test]
    fn bare_metal_cpuinfo_not_flagged() {
        let (_dir, root) = quiet_root();
        fs::write(
            root.join("proc/cpuinfo"),
            "processor : 0\nflags : fpu vme sse2\n",
        )
        .unwrap();

        assert!(!detect(&root).contains(&PreflightWarning::VirtualizedHost));
    }

    #[test]
    fn detects_dmi_vendor() {
        let (_dir, root) = quiet_root();
        fs::create_dir_all(root.join("sys/devices/virtual/dmi/id")).unwrap();
        fs::write(root.join("sys/devices/virtual/dmi/id/sys_vendor"), "QEMU\n").unwrap();

        assert!(detect(&root).contains(&PreflightWarning::VirtualizedHost));
    }

    #[test]
    fn detects_dmi_product_name() {
        let (_dir, root) = quiet_root();
        fs::create_dir_all(root.join("sys/devices/virtual/dmi/id")).unwrap();
        fs::write(
            root.join("sys/devices/virtual/dmi/id/sys_vendor"),
            "Microsoft Corporation\n",
        )
        .unwrap();
        fs::write(
            root.join("sys/devices/virtual/dmi/id/product_name"),
            "Virtual Machine\n",
        )
        .unwrap();

        assert!(detect(&root).contains(&PreflightWarning::VirtualizedHost));
    }

    #[test]
    fn detects_missing_isolation_boot_args() {
        let (_dir, root) = quiet_root();
        fs::write(root.join("proc/cmdline"), "root=/dev/sda1 quiet\n").unwrap();

        assert!(detect(&root).contains(&PreflightWarning::NoIsolationBootArgs));
    }

    #[test]
    fn nohz_full_alone_counts_as_isolation() {
        let (_dir, root) = quiet_root();
        fs::write(root.join("proc/cmdline"), "nohz_full=2-7\n").unwrap();

        assert!(!detect(&root).contains(&PreflightWarning::NoIsolationBootArgs));
    }

    #[test]
    fn unreadable_cmdline_gives_no_boot_arg_warning() {
        let (_dir, root) = quiet_root();
        fs::remove_file(root.join("proc/cmdline")).unwrap();

        assert!(!detect(&root).contains(&PreflightWarning::NoIsolationBootArgs));
    }

    #[test]
    fn warning_display_is_human_readable() {
        assert!(
            PreflightWarning::IrqBalanceRunning
                .to_string()
                .contains("irqbalance")
        );
        assert!(
            PreflightWarning::NoCgroupV2
                .to_string()
                .contains("cgroup v2")
        );
        assert!(
            PreflightWarning::VirtualizedHost
                .to_string()
                .contains("virtualized")
        );
        assert!(
            PreflightWarning::NoIsolationBootArgs
                .to_string()
                .contains("isolcpus")
        );
    }
}
