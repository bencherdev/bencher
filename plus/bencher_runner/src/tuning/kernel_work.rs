//! Steer device IRQs and unbound kernel workqueues to housekeeping cores.
//!
//! Device interrupts and unbound workqueue workers landing on benchmark
//! cores steal cycles mid-measurement. This module points the IRQ default
//! affinity, every movable IRQ, and the unbound workqueue cpumask at the
//! housekeeping cores instead. Some IRQs are not movable (the kernel
//! returns EIO); those are skipped and summarized in a single line.

use camino::Utf8Path;

use super::write_sysctl;
use crate::cpu::{CpuLayout, format_cpumask};

/// Steer kernel work (IRQs and unbound workqueues) to housekeeping cores.
///
/// `root` is the filesystem root (`/` in production); tests pass a
/// tempdir tree containing `proc/` and `sys/` subtrees.
pub(super) fn steer_kernel_work(layout: &CpuLayout, root: &Utf8Path) {
    let housekeeping_mask = format_cpumask(&layout.housekeeping);

    // New IRQs default to housekeeping cores.
    write_sysctl(
        root.join("proc/irq/default_smp_affinity").as_str(),
        &housekeeping_mask,
        "default IRQ affinity",
    );

    // Unbound workqueue workers run on housekeeping cores.
    write_sysctl(
        root.join("sys/devices/virtual/workqueue/cpumask").as_str(),
        &housekeeping_mask,
        "workqueue cpumask",
    );

    steer_existing_irqs(layout, root);
}

/// Move every movable IRQ to the housekeeping cores.
///
/// Iterates `proc/irq/<N>/smp_affinity_list`, skipping per-IRQ failures
/// (unmovable IRQs fail with EIO), and prints one summary line.
fn steer_existing_irqs(layout: &CpuLayout, root: &Utf8Path) {
    let irq_dir = root.join("proc/irq");
    let Ok(entries) = std::fs::read_dir(irq_dir.as_std_path()) else {
        println!("  Tuning: IRQ steering - skipped (cannot read {irq_dir})");
        return;
    };

    let housekeeping_list = layout.housekeeping_cpuset();
    let mut total = 0usize;
    let mut moved = 0usize;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        // Only numeric directories are IRQs (skips default_smp_affinity etc.)
        if name_str.is_empty() || !name_str.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }

        let affinity_path = irq_dir.join(name_str).join("smp_affinity_list");
        let Ok(current) = std::fs::read_to_string(affinity_path.as_std_path()) else {
            continue;
        };
        total += 1;

        if current.trim() == housekeeping_list {
            moved += 1;
            continue;
        }

        if std::fs::write(affinity_path.as_std_path(), &housekeeping_list).is_err() {
            // Unmovable IRQ (EIO) or insufficient permissions - skip.
            continue;
        }

        moved += 1;
    }

    println!(
        "  Tuning: IRQ steering - moved {moved} of {total} IRQs to housekeeping cores ({housekeeping_list})"
    );
}

#[cfg(test)]
mod tests {
    use std::fs;

    use camino::Utf8PathBuf;

    use super::*;

    /// Build a fake `/proc` + `/sys` tree with two movable IRQs.
    fn fake_root() -> (tempfile::TempDir, Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        fs::create_dir_all(root.join("proc/irq/10")).unwrap();
        fs::create_dir_all(root.join("proc/irq/11")).unwrap();
        fs::create_dir_all(root.join("sys/devices/virtual/workqueue")).unwrap();

        fs::write(root.join("proc/irq/default_smp_affinity"), "ff").unwrap();
        fs::write(root.join("proc/irq/10/smp_affinity_list"), "0-7\n").unwrap();
        fs::write(root.join("proc/irq/11/smp_affinity_list"), "0-7\n").unwrap();
        fs::write(root.join("sys/devices/virtual/workqueue/cpumask"), "ff").unwrap();

        (dir, root)
    }

    #[test]
    fn steers_irqs_and_workqueues_to_housekeeping() {
        let (_dir, root) = fake_root();
        let layout = CpuLayout::with_core_count(8);

        steer_kernel_work(&layout, &root);

        assert_eq!(
            fs::read_to_string(root.join("proc/irq/default_smp_affinity")).unwrap(),
            "3"
        );
        assert_eq!(
            fs::read_to_string(root.join("sys/devices/virtual/workqueue/cpumask")).unwrap(),
            "3"
        );
        assert_eq!(
            fs::read_to_string(root.join("proc/irq/10/smp_affinity_list")).unwrap(),
            "0-1"
        );
        assert_eq!(
            fs::read_to_string(root.join("proc/irq/11/smp_affinity_list")).unwrap(),
            "0-1"
        );
    }

    #[test]
    fn skips_unmovable_irq() {
        let (_dir, root) = fake_root();
        // A directory in place of the affinity file forces the write to
        // fail, mimicking an unmovable IRQ (EIO) even when running as root.
        fs::create_dir_all(root.join("proc/irq/12/smp_affinity_list")).unwrap();
        let layout = CpuLayout::with_core_count(8);

        steer_kernel_work(&layout, &root);

        // The unmovable IRQ is skipped; the movable ones are still steered.
        for irq in ["10", "11"] {
            assert_eq!(
                fs::read_to_string(root.join(format!("proc/irq/{irq}/smp_affinity_list"))).unwrap(),
                "0-1"
            );
        }
    }

    #[test]
    fn a_restart_leaves_a_steered_irq_unwritten() {
        // Kills dropping the equality check: the kernel prints a newline the
        // runner never writes, so a rewrite loses it.
        let (_dir, root) = fake_root();
        fs::write(root.join("proc/irq/10/smp_affinity_list"), "0-1\n").unwrap();
        let layout = CpuLayout::with_core_count(8);

        steer_kernel_work(&layout, &root);

        assert_eq!(
            fs::read_to_string(root.join("proc/irq/10/smp_affinity_list")).unwrap(),
            "0-1\n"
        );
    }

    #[test]
    fn ignores_non_numeric_irq_dirs() {
        let (_dir, root) = fake_root();
        fs::create_dir_all(root.join("proc/irq/not-an-irq")).unwrap();
        fs::write(root.join("proc/irq/not-an-irq/smp_affinity_list"), "0-7\n").unwrap();
        let layout = CpuLayout::with_core_count(8);

        steer_kernel_work(&layout, &root);

        assert_eq!(
            fs::read_to_string(root.join("proc/irq/not-an-irq/smp_affinity_list")).unwrap(),
            "0-7\n"
        );
    }
}
