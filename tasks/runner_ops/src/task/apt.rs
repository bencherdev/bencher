pub const AUTOREMOVE_SIMULATION: &str = "LC_ALL=C apt-get -s autoremove";
/// The last line of a security updates query that ran to its end.
pub const SECURITY_UPDATES_END: &str = "checked";

/// The most days the signed `Date:` of Ubuntu's own security pocket may age, by the operator's clock, before the security updates query fails.
const SECURITY_LISTS_MAX_AGE_DAYS: u64 = 7;

/// Each installed package with a newer version in a `-security` archive, or not fully installed, as `<package> <version>`, failing unless the lists in use hold Ubuntu's own security pocket signed at most `SECURITY_LISTS_MAX_AGE_DAYS` days before `now_unix`.
pub fn security_updates_query(now_unix: u64) -> String {
    format!(
        r#"python3 -I -c '
import apt_pkg, glob, os, platform
apt_pkg.init()
apt_pkg.config.set("Dir::Cache::pkgcache", "")
apt_pkg.config.set("Dir::Cache::srcpkgcache", "")
cache = apt_pkg.Cache(None)
suite = platform.freedesktop_os_release()["VERSION_CODENAME"] + "-security"
used = set()
for pf in cache.file_list:
    if pf.origin == "Ubuntu" and pf.archive == suite:
        tail = apt_pkg.uri_to_filename("file:/" + pf.component + "/binary-" + pf.architecture + "/Packages")
        if pf.filename.endswith(tail):
            used.add(pf.filename[:-len(tail)])
dates = []
lists = apt_pkg.config.find_dir("Dir::State::lists")
for path in glob.glob(lists + "*_InRelease") + glob.glob(lists + "*_Release"):
    if path[:path.rindex("_")] not in used:
        continue
    with os.fdopen(apt_pkg.open_maybe_clear_signed_file(path)) as body:
        release = next(iter(apt_pkg.TagFile(body)), None) or dict()
    if release.get("Origin") == "Ubuntu" and release.get("Suite") == suite:
        dates.append((apt_pkg.str_to_time(release.get("Date", "")) or 0, release.get("Date")))
if not dates:
    raise SystemExit("no package list in use comes from the Ubuntu " + suite + " pocket")
stamp, date = max(dates, key=lambda dated: dated[0])
if not stamp:
    raise SystemExit("the Ubuntu " + suite + " Release file has no valid signed Date field")
if {now_unix} - stamp > {SECURITY_LISTS_MAX_AGE_DAYS} * 86400:
    raise SystemExit("the Ubuntu " + suite + " Release file is signed with the date " + date + ", more than {SECURITY_LISTS_MAX_AGE_DAYS} days before the audit")
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
