pub const AUTOREMOVE_SIMULATION: &str = "LC_ALL=C apt-get -s autoremove";
/// The last line of a security updates query that ran to its end.
pub const SECURITY_UPDATES_END: &str = "checked";

/// The most days the newest `Release` file of a `-security` archive may age before the security updates query fails.
const SECURITY_LISTS_MAX_AGE_DAYS: u32 = 7;

/// Each installed package with a newer version in an archive whose name ends in `-security`, or that dpkg has not finished installing, as `<package> <version>`, read from the package lists and the dpkg status, so no apt preference, hold, phasing, or dependency problem can hide one.
/// It builds its cache in memory, so a broken list fails every run and nothing lands in `/var/cache/apt`.
pub fn security_updates_query() -> String {
    format!(
        r#"python3 -I -c '
import apt_pkg, glob, time
apt_pkg.init()
apt_pkg.config.set("Dir::Cache::pkgcache", "")
apt_pkg.config.set("Dir::Cache::srcpkgcache", "")
cache = apt_pkg.Cache(None)
if not any((pf.archive or "").endswith("-security") for pf in cache.file_list):
    raise SystemExit("no package list comes from a -security archive")
latest = None
for path in glob.glob(apt_pkg.config.find_dir("Dir::State::lists") + "*Release"):
    suite = date = None
    with open(path, errors="replace") as release:
        for line in release:
            if line.startswith("Suite: ") and suite is None:
                suite = line[7:].strip()
            elif line.startswith("Date: ") and date is None:
                date = line[6:].strip()
    if (suite or "").endswith("-security"):
        stamp = apt_pkg.str_to_time(date or "") or 0
        if latest is None or stamp > latest[0]:
            latest = (stamp, date)
if latest is None:
    raise SystemExit("no Release file comes from a -security archive")
if latest[1] is None:
    raise SystemExit("the newest -security Release file has no Date field")
if time.time() - latest[0] > {SECURITY_LISTS_MAX_AGE_DAYS} * 86400:
    raise SystemExit("the newest -security Release file is dated " + str(latest[1]) + ", more than {SECURITY_LISTS_MAX_AGE_DAYS} days ago")
rows = []
for pkg in cache.packages:
    installed = pkg.current_ver
    if installed and pkg.current_state != apt_pkg.CURSTATE_INSTALLED:
        rows.append(pkg.get_fullname(True) + " " + installed.ver_str)
        continue
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
        assert_eq!(
            missing_security_updates("Warning\nchecked\n"),
            Err("Warning")
        );
    }
}
