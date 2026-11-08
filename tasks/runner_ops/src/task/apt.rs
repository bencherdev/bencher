pub const AUTOREMOVE_SIMULATION: &str = "LC_ALL=C apt-get -s autoremove";
/// The last line of a security updates query that ran to its end.
pub const SECURITY_UPDATES_END: &str = "checked";

/// Each installed package with a newer version in an archive whose name ends in `-security`, as `<package> <version>`, read from the package lists alone, so no apt preference, hold, phasing, or dependency problem can hide one.
pub fn security_updates_query() -> String {
    format!(
        r#"python3 -I -c '
import apt_pkg
apt_pkg.init()
rows = []
for pkg in apt_pkg.Cache(None).packages:
    installed = pkg.current_ver
    newest = None
    for ver in pkg.version_list if installed else []:
        if apt_pkg.version_compare(ver.ver_str, newest or installed.ver_str) > 0 and any((pf.archive or "").endswith("-security") for pf, _ in ver.file_list):
            newest = ver.ver_str
    if newest:
        rows.append(pkg.get_fullname(True) + " " + newest)
for row in sorted(rows):
    print(row)
print("{SECURITY_UPDATES_END}")'"#
    )
}

/// Packages that `apt-get -s autoremove` would remove.
pub fn autoremovable_packages(simulation: &str) -> Vec<&str> {
    simulation
        .lines()
        .filter_map(|line| line.strip_prefix("Remv "))
        .filter_map(|rest| rest.split_whitespace().next())
        .collect()
}

/// Each package and version the security updates query lists, or the line that shows it did not run to its end.
pub fn missing_security_updates(output: &str) -> Result<Vec<(&str, &str)>, &str> {
    let mut lines: Vec<&str> = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    match lines.pop() {
        Some(SECURITY_UPDATES_END) => {},
        last => return Err(last.unwrap_or("no output")),
    }
    lines
        .into_iter()
        .map(|line| {
            let mut fields = line.split_whitespace();
            match (fields.next(), fields.next(), fields.next()) {
                (Some(package), Some(version), None) => Ok((package, version)),
                _ => Err(line),
            }
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
    fn missing_security_updates_lists_each_package() {
        let output = "\
libc6:i386 2.39-0ubuntu8.7
linux-generic 6.8.0-147.147
openssh-server 1:9.6p1-3ubuntu13.20
checked
";
        assert_eq!(
            missing_security_updates(output),
            Ok(vec![
                ("libc6:i386", "2.39-0ubuntu8.7"),
                ("linux-generic", "6.8.0-147.147"),
                ("openssh-server", "1:9.6p1-3ubuntu13.20"),
            ])
        );
        assert_eq!(missing_security_updates("checked\n"), Ok(Vec::new()));
    }

    #[test]
    fn missing_security_updates_fails_without_its_end() {
        let traceback = "\
Traceback (most recent call last):
  File \"<string>\", line 2, in <module>
ModuleNotFoundError: No module named 'apt_pkg'
";
        assert_eq!(
            missing_security_updates(traceback),
            Err("ModuleNotFoundError: No module named 'apt_pkg'")
        );
        assert_eq!(
            missing_security_updates("bash: line 1: python3: command not found\n"),
            Err("bash: line 1: python3: command not found")
        );
        assert_eq!(missing_security_updates(""), Err("no output"));
        assert_eq!(
            missing_security_updates("linux-generic 6.8.0-147.147\n"),
            Err("linux-generic 6.8.0-147.147")
        );
        assert_eq!(
            missing_security_updates("W: an unexpected line\nchecked\n"),
            Err("W: an unexpected line")
        );
    }
}
