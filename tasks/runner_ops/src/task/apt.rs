pub const AUTOREMOVE_SIMULATION: &str = "LC_ALL=C apt-get -s autoremove";
/// A full upgrade that prefers the security pockets and ignores apt preferences and holds, so no pin or hold can hide a security update.
pub const SECURITY_UPGRADE_SIMULATION: &str = "LC_ALL=C apt-get -s -o Dir::Etc::Preferences=/dev/null -o Dir::Etc::PreferencesParts=/dev/null -o 'APT::Default-Release=*-security' -o APT::Ignore-Hold=true dist-upgrade";

/// Packages that `apt-get -s autoremove` would remove.
pub fn autoremovable_packages(simulation: &str) -> Vec<&str> {
    simulation
        .lines()
        .filter_map(|line| line.strip_prefix("Remv "))
        .filter_map(|rest| rest.split_whitespace().next())
        .collect()
}

/// Each installed package and the version an `apt-get -s` simulation upgrades it to from a security pocket.
pub fn security_upgrades(simulation: &str) -> Vec<(&str, &str)> {
    simulation
        .lines()
        .filter_map(|line| {
            // `Inst <package> [<installed>] (<version> <origin>, ... [<arch>])`, with no `[<installed>]` for a new package; an origin is `<label>:<release>/<archive>`, or `<label>:<archive>` when the release has no version.
            let (package, rest) = line.strip_prefix("Inst ")?.split_once(' ')?;
            let (_, candidate) = rest.strip_prefix('[')?.split_once("] (")?;
            let (candidate, _) = candidate.split_once(')')?;
            let mut fields = candidate.split_whitespace();
            let version = fields.next()?;
            fields
                .any(|origin| origin.trim_end_matches(',').ends_with("-security"))
                .then_some((package, version))
        })
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

    #[test]
    fn security_upgrades_lists_installed_packages_from_a_security_pocket() {
        let simulation = "\
NOTE: This is only a simulation!
Reading package lists...
16 upgraded, 6 newly installed, 0 to remove and 0 not upgraded.
Inst libapparmor1 [4.0.1really4.0.1-0ubuntu0.24.04.8] (4.0.1really4.0.1-0ubuntu0.24.04.9 Ubuntu:24.04/noble-updates [amd64]) []
Inst linux-image-6.8.0-147-generic (6.8.0-147.147 Ubuntu:24.04/noble-updates, Ubuntu:24.04/noble-security [amd64])
Inst linux-image-virtual [6.8.0-142.142] (6.8.0-147.147 Ubuntu:24.04/noble-security [amd64]) []
Inst linux-tools-common [6.8.0-142.142] (6.8.0-147.147 Ubuntu:24.04/noble-updates, Ubuntu:24.04/noble-security [all])
Inst linux-libc-dev [6.8.0-142.142] (6.8.0-147.147 Ubuntu:24.04/noble-security, Ubuntu:24.04/noble-updates [amd64]) []
Inst linux-base [4.5ubuntu9] (4.5ubuntu9+24.04.1 Ubuntu:noble-security [all])
Inst tmux [3.4-1ubuntu0.1] (3.4-1ubuntu0.2 Ubuntu:24.04/noble-updates [amd64])
Conf linux-image-virtual (6.8.0-147.147 Ubuntu:24.04/noble-security [amd64])
";
        assert_eq!(
            security_upgrades(simulation),
            [
                ("linux-image-virtual", "6.8.0-147.147"),
                ("linux-tools-common", "6.8.0-147.147"),
                ("linux-libc-dev", "6.8.0-147.147"),
                ("linux-base", "4.5ubuntu9+24.04.1"),
            ]
        );
    }
}
