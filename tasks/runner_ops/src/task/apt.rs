pub const AUTOREMOVE_SIMULATION: &str = "LC_ALL=C apt-get -s autoremove";

/// Packages that `apt-get -s autoremove` would remove.
pub fn autoremovable_packages(simulation: &str) -> Vec<&str> {
    simulation
        .lines()
        .filter_map(|line| line.strip_prefix("Remv "))
        .filter_map(|rest| rest.split_whitespace().next())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autoremovable_packages_lists_removals() {
        let simulation = "\
NOTE: This is only a simulation!
Reading package lists...
The following packages will be REMOVED:
  bpftrace libclang-cpp18
0 upgraded, 0 newly installed, 2 to remove and 0 not upgraded.
Remv bpftrace [0.20.2-1ubuntu4]
Remv libclang-cpp18 [1:18.1.3-1ubuntu1]
";
        assert_eq!(
            autoremovable_packages(simulation),
            ["bpftrace", "libclang-cpp18"]
        );
    }

    #[test]
    fn autoremovable_packages_none() {
        let simulation = "\
NOTE: This is only a simulation!
Reading package lists...
0 upgraded, 0 newly installed, 0 to remove and 0 not upgraded.
";
        assert_eq!(autoremovable_packages(simulation), Vec::<&str>::new());
    }
}
