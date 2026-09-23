use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write as _};

use bencher_json::RunnerKey;

use crate::task::apt::AUTOREMOVE_SIMULATION;
use crate::task::harden::{AUTO_UPGRADES_PATH, SSH_HARDENING_PATH, UNATTENDED_UPGRADES_PATH};

const MARKER: &str = "@@audit ";
const STATUS: &str = "@@status ";
const RUNNER_KEY: &str = "BENCHER_RUNNER_KEY";
const RUNNER_NAME: &str = "BENCHER_RUNNER=";
const RUNNER_UNIT: &str = "/etc/systemd/system/bencher-runner.service";
// The unit lines runner ops writes and the privileges the runner depends on; `BENCHER_RUNNER=` cannot match the key.
const RUNNER_UNIT_LINES: &str = r"^(\[|Description=|After=|Wants=|Type=|ExecStart=|Restart=|RestartSec=|WantedBy=|User=|Delegate=|EnvironmentFile=|Environment=BENCHER_(HOST|RUNNER|UPDATE_CHANNEL|DANGER_ALLOW_NO_SANDBOX)=)";
const INSTALLED_PACKAGES: &str =
    r"dpkg-query -W -f='${db:Status-Status} ${binary:Package} ${Version}\n'";
const KEEP_INSTALLED: &str = r#"awk '$1 == "installed" { print $2, $3 }'"#;

/// One part of a runner audit, filled by one read-only command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Section {
    Os,
    Kernel,
    KernelPackages,
    Cmdline,
    Hardware,
    Smt,
    Raid,
    RunnerBinary,
    RunnerUnit,
    Sshd,
    Apt,
    Grub,
    Ufw,
    EnabledUnits,
    Timezone,
    Packages,
    ManualPackages,
    RunnerService,
    RebootRequired,
    Autoremove,
}

impl Section {
    pub const ALL: [Self; 20] = [
        Self::Os,
        Self::Kernel,
        Self::KernelPackages,
        Self::Cmdline,
        Self::Hardware,
        Self::Smt,
        Self::Raid,
        Self::RunnerBinary,
        Self::RunnerUnit,
        Self::Sshd,
        Self::Apt,
        Self::Grub,
        Self::Ufw,
        Self::EnabledUnits,
        Self::Timezone,
        Self::Packages,
        Self::ManualPackages,
        Self::RunnerService,
        Self::RebootRequired,
        Self::Autoremove,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::Os => "os",
            Self::Kernel => "kernel",
            Self::KernelPackages => "kernel packages",
            Self::Cmdline => "cmdline",
            Self::Hardware => "hardware",
            Self::Smt => "smt",
            Self::Raid => "raid",
            Self::RunnerBinary => "runner binary",
            Self::RunnerUnit => "runner unit",
            Self::Sshd => "sshd",
            Self::Apt => "apt",
            Self::Grub => "grub",
            Self::Ufw => "ufw",
            Self::EnabledUnits => "enabled units",
            Self::Timezone => "timezone",
            Self::Packages => "packages",
            Self::ManualPackages => "manual packages",
            Self::RunnerService => "runner service",
            Self::RebootRequired => "reboot required",
            Self::Autoremove => "autoremove",
        }
    }

    fn command(self) -> String {
        match self {
            Self::Os => "cat /etc/os-release".to_owned(),
            Self::Kernel => "uname -r".to_owned(),
            Self::KernelPackages => format!("{INSTALLED_PACKAGES} 'linux-*' | {KEEP_INSTALLED}"),
            Self::Cmdline => "cat /proc/cmdline".to_owned(),
            Self::Hardware => "grep -m1 '^model name' /proc/cpuinfo && grep -m1 '^microcode' /proc/cpuinfo && grep -H . /sys/class/dmi/id/bios_version /sys/class/dmi/id/board_name".to_owned(),
            Self::Smt => "grep -H . /sys/devices/system/cpu/smt/control /sys/devices/system/cpu/online".to_owned(),
            Self::Raid => "cat /proc/mdstat".to_owned(),
            Self::RunnerBinary => "sha256sum /usr/local/bin/runner".to_owned(),
            Self::RunnerUnit => runner_unit_command(RUNNER_UNIT),
            Self::Sshd => format!("grep -H . {SSH_HARDENING_PATH}"),
            Self::Apt => format!("grep -H . {UNATTENDED_UPGRADES_PATH} {AUTO_UPGRADES_PATH}"),
            Self::Grub => "grep -H . /etc/default/grub.d/*.cfg".to_owned(),
            Self::Ufw => "ufw status verbose".to_owned(),
            Self::EnabledUnits => {
                "systemctl list-unit-files --state=enabled --no-legend | awk '{ print $1 }'"
                    .to_owned()
            },
            Self::Timezone => "timedatectl show -p Timezone --value".to_owned(),
            Self::Packages => format!("{INSTALLED_PACKAGES} | {KEEP_INSTALLED}"),
            Self::ManualPackages => "apt-mark showmanual".to_owned(),
            Self::RunnerService => "systemctl is-active bencher-runner".to_owned(),
            Self::RebootRequired => "if [ -e /var/run/reboot-required ]; then echo required; cat /var/run/reboot-required.pkgs 2>/dev/null || true; else echo none; fi".to_owned(),
            Self::Autoremove => AUTOREMOVE_SIMULATION.to_owned(),
        }
    }

    /// Whether the section must match across runners, rather than only feeding a health check.
    pub fn in_snapshot(self) -> bool {
        !matches!(
            self,
            Self::RunnerService | Self::RebootRequired | Self::Autoremove
        )
    }

    /// `systemctl is-active` exits non-zero for an inactive unit, which its own check reports.
    pub fn expects_success(self) -> bool {
        self != Self::RunnerService
    }

    /// Sections whose line order carries no meaning, so their lines are sorted before comparing.
    fn sorted(self) -> bool {
        matches!(
            self,
            Self::KernelPackages | Self::EnabledUnits | Self::Packages | Self::ManualPackages
        )
    }

    /// Whether a reorder of the same lines is a difference; RAID arrays are sorted as blocks.
    fn order_matters(self) -> bool {
        !self.sorted() && self != Self::Raid
    }

    /// Normalize the values that legitimately differ between runners.
    pub fn normalize(self, raw: &str) -> Vec<String> {
        let lines = raw
            .lines()
            .map(str::trim_end)
            .filter(|line| !line.is_empty());
        let mut lines: Vec<String> = match self {
            Self::Cmdline => lines.map(normalize_cmdline).collect(),
            Self::Raid => normalize_mdstat(lines),
            Self::RunnerUnit => lines.map(normalize_runner_name).collect(),
            Self::KernelPackages
            | Self::EnabledUnits
            | Self::Packages
            | Self::ManualPackages
            | Self::Os
            | Self::Kernel
            | Self::Hardware
            | Self::Smt
            | Self::RunnerBinary
            | Self::Sshd
            | Self::Apt
            | Self::Grub
            | Self::Ufw
            | Self::Timezone
            | Self::RunnerService
            | Self::RebootRequired
            | Self::Autoremove => lines.map(str::to_owned).collect(),
        };
        if self.sorted() {
            lines.sort_unstable();
        }
        lines
    }
}

impl fmt::Display for Section {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The remote script that prints each section's output between a marker line and its exit status.
pub fn script() -> String {
    let mut script =
        String::from("(set -o pipefail) 2>/dev/null && set -o pipefail\nexport LC_ALL=C\n");
    for section in Section::ALL {
        _ = writeln!(
            script,
            "{}{{ {}; }} 2>&1",
            marker(section),
            section.command()
        );
        _ = writeln!(script, "printf '\\n{STATUS}%s\\n' \"$?\"");
    }
    // A failing section command is audit data; only a failed connection is an error.
    script.push_str("exit 0\n");
    script
}

// The leading newline keeps a marker off the last line of output that has no trailing newline.
fn marker(section: Section) -> String {
    format!("printf '\\n{MARKER}%s\\n' '{}'\n", section.name())
}

/// Only allowlisted lines of the unit and its drop-ins, never `systemctl cat`, and no line holding a runner key leaves the server.
fn runner_unit_command(unit: &str) -> String {
    format!(
        "grep -H -E '{RUNNER_UNIT_LINES}' {unit} {unit}.d/*.conf | grep -v {}",
        RunnerKey::PREFIX
    )
}

/// Raw section output and exit status from one runner, with every runner key line removed.
#[derive(Debug)]
pub struct Sections(BTreeMap<Section, Output>);

#[derive(Debug, Default)]
struct Output {
    text: String,
    status: Option<i32>,
}

impl Sections {
    pub fn parse(output: &str) -> Self {
        let mut sections = BTreeMap::<Section, Output>::new();
        let mut current = None;
        for line in output.lines() {
            if let Some(name) = line.strip_prefix(MARKER) {
                current = Section::ALL
                    .into_iter()
                    .find(|section| section.name() == name);
                if let Some(section) = current {
                    sections.entry(section).or_default();
                }
            } else if let Some(section) = current
                && !line.contains(RUNNER_KEY)
                && !line.contains(RunnerKey::PREFIX)
            {
                let output = sections.entry(section).or_default();
                if let Some(status) = line.strip_prefix(STATUS) {
                    output.status = status.parse().ok();
                } else {
                    output.text.push_str(line);
                    output.text.push('\n');
                }
            }
        }
        Self(sections)
    }

    pub fn get(&self, section: Section) -> &str {
        self.0
            .get(&section)
            .map_or("", |output| output.text.as_str())
    }

    /// The section's exit status, or `None` if it never finished.
    pub fn status(&self, section: Section) -> Option<i32> {
        self.0.get(&section).and_then(|output| output.status)
    }
}

/// Normalized sections that should be identical across runners.
#[derive(Debug, PartialEq, Eq)]
pub struct Snapshot(BTreeMap<Section, Vec<String>>);

impl Snapshot {
    pub fn new(sections: &Sections) -> Self {
        Self(
            Section::ALL
                .into_iter()
                .filter(|section| section.in_snapshot())
                .map(|section| {
                    let mut lines = section.normalize(sections.get(section));
                    match sections.status(section) {
                        Some(0) => {},
                        Some(status) => lines.push(format!("exit status: {status}")),
                        None => lines.push("exit status: missing".to_owned()),
                    }
                    (section, lines)
                })
                .collect(),
        )
    }

    /// The number of sections and lines compared.
    pub fn size(&self) -> (usize, usize) {
        (self.0.len(), self.0.values().map(Vec::len).sum())
    }

    /// The lines of each section that appear in only one of the two snapshots.
    pub fn diff<'a>(&'a self, reference: &'a Self) -> Vec<SectionDiff<'a>> {
        self.0
            .iter()
            .map(|(&section, lines)| {
                let reference_lines = reference.0.get(&section).map_or(&[][..], Vec::as_slice);
                let reference_only = only_in(reference_lines, lines);
                let runner_only = only_in(lines, reference_lines);
                let order_differs = section.order_matters()
                    && reference_only.is_empty()
                    && runner_only.is_empty()
                    && lines.as_slice() != reference_lines;
                SectionDiff {
                    section,
                    reference: reference_only,
                    runner: runner_only,
                    order_differs,
                }
            })
            .filter(|diff| {
                !diff.reference.is_empty() || !diff.runner.is_empty() || diff.order_differs
            })
            .collect()
    }
}

impl fmt::Display for Snapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (section, lines) in &self.0 {
            writeln!(f, "[{section}]")?;
            for line in lines {
                writeln!(f, "{line}")?;
            }
        }
        Ok(())
    }
}

/// Lines only the reference runner has (`-`), lines only the audited runner has (`+`),
/// and whether the same lines come in a different order or count (`~`).
#[derive(Debug, PartialEq, Eq)]
pub struct SectionDiff<'a> {
    section: Section,
    reference: Vec<&'a str>,
    runner: Vec<&'a str>,
    order_differs: bool,
}

impl fmt::Display for SectionDiff<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            section,
            reference,
            runner,
            order_differs,
        } = self;
        writeln!(f, "[{section}]")?;
        for line in reference {
            writeln!(f, "- {line}")?;
        }
        for line in runner {
            writeln!(f, "+ {line}")?;
        }
        if *order_differs {
            writeln!(f, "~ order differs")?;
        }
        Ok(())
    }
}

fn only_in<'a>(lines: &'a [String], other: &[String]) -> Vec<&'a str> {
    let other: BTreeSet<&str> = other.iter().map(String::as_str).collect();
    lines
        .iter()
        .map(String::as_str)
        .filter(|line| !other.contains(line))
        .collect()
}

fn normalize_cmdline(line: &str) -> String {
    line.split_whitespace()
        .map(|arg| {
            if arg.starts_with("root=UUID=") {
                "root=UUID=<root>"
            } else {
                arg
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Keep each array's line and its size line (prefixed with the array name) together, sorted by
/// array number, because the kernel lists arrays in the order it assembled them at boot.
/// Progress and bitmap lines are left out.
fn normalize_mdstat<'a, I>(lines: I) -> Vec<String>
where
    I: Iterator<Item = &'a str>,
{
    let mut arrays: Vec<(Option<u32>, String, Vec<String>)> = Vec::new();
    for line in lines {
        if let Some(array) = normalize_raid_array(line) {
            let name = array
                .split_once(' ')
                .map_or_else(|| array.clone(), |(name, _)| name.to_owned());
            let number = name
                .strip_prefix("md")
                .and_then(|number| number.parse().ok());
            arrays.push((number, name, vec![array]));
        } else if line.contains(" blocks ")
            && let Some((_, name, array_lines)) = arrays.last_mut()
        {
            let size = line.split_whitespace().collect::<Vec<_>>().join(" ");
            array_lines.push(format!("{name}: {size}"));
        }
    }
    arrays.sort_by(|(a_number, _, a_lines), (b_number, _, b_lines)| {
        a_number.cmp(b_number).then_with(|| a_lines.cmp(b_lines))
    });
    arrays.into_iter().flat_map(|(_, _, lines)| lines).collect()
}

/// Drop member role numbers and sort members, keeping flags such as `(S)` and `(F)`,
/// so `md0 : active raid1 nvme1n1p1[1] nvme0n1p1[0]` matches either member order.
fn normalize_raid_array(line: &str) -> Option<String> {
    let (name, _) = line.split_once(" : ")?;
    if !name.starts_with("md") {
        return None;
    }
    let (members, fields): (Vec<&str>, Vec<&str>) = line
        .split_whitespace()
        .partition(|field| field.contains('['));
    let mut members: Vec<String> = members
        .into_iter()
        .map(|member| match member.split_once('[') {
            Some((device, rest)) => {
                let flags = rest.split_once(']').map_or("", |(_, flags)| flags);
                format!("{device}{flags}")
            },
            None => member.to_owned(),
        })
        .collect();
    members.sort_unstable();
    let mut fields: Vec<String> = fields.into_iter().map(str::to_owned).collect();
    fields.append(&mut members);
    Some(fields.join(" "))
}

fn normalize_runner_name(line: &str) -> String {
    let Some((head, rest)) = line.split_once(RUNNER_NAME) else {
        return line.to_owned();
    };
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '"')
        .unwrap_or(rest.len());
    let (_, tail) = rest.split_at(end);
    format!("{head}{RUNNER_NAME}<runner>{tail}")
}

/// Fake `script()` output for tests, where every section succeeds.
#[cfg(test)]
pub fn fake_output(sections: &[(Section, &str)]) -> String {
    sections
        .iter()
        .map(|&(section, text)| fake_section(section, text, Some(0)))
        .collect()
}

/// Fake `script()` output for one section, cut off before its exit status when `status` is `None`.
#[cfg(test)]
pub fn fake_section(section: Section, text: &str, status: Option<i32>) -> String {
    let status = status.map_or_else(String::new, |status| format!("\n{STATUS}{status}\n"));
    format!("\n{MARKER}{}\n{text}\n{status}", section.name())
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;

    const KEY: &str = "bencher_runner_0123456789abcdef";

    fn runner_output(name: &str, root: &str, members: [&str; 2], kernel: &str) -> String {
        let [first, second] = members;
        fake_output(&[
            (Section::Kernel, kernel),
            (
                Section::Cmdline,
                &format!("BOOT_IMAGE=/vmlinuz-{kernel} root=UUID={root} ro isolcpus=1-5"),
            ),
            (
                Section::Raid,
                &format!(
                    "Personalities : [raid1]\nmd2 : active raid1 {first} {second}\n      1000 blocks super 1.2 [2/2] [UU]\n\nunused devices: <none>"
                ),
            ),
            (
                Section::RunnerUnit,
                &format!(
                    "{RUNNER_UNIT}:ExecStart=/usr/local/bin/runner up --key {KEY}\n{RUNNER_UNIT}.d/credentials.conf:[Service]\n{RUNNER_UNIT}.d/credentials.conf:Environment=BENCHER_RUNNER={name}\n{RUNNER_UNIT}.d/credentials.conf:Environment=BENCHER_RUNNER_KEY={KEY}"
                ),
            ),
            (Section::Packages, "zlib1g 1:1.3\ncurl 8.5.0"),
            (Section::RunnerService, "active"),
        ])
    }

    fn snapshot(output: &str) -> Snapshot {
        Snapshot::new(&Sections::parse(output))
    }

    #[test]
    fn script_prints_every_section_and_status() {
        let script = script();
        for section in Section::ALL {
            assert!(script.contains(&marker(section)), "{section}");
        }
        assert_eq!(
            script.matches("printf '\\n@@status %s\\n' \"$?\"").count(),
            Section::ALL.len()
        );
        assert!(script.ends_with("exit 0\n"));
    }

    #[test]
    fn sections_parse_by_marker() {
        let sections = Sections::parse(&fake_output(&[
            (Section::Kernel, "6.8.0-90-generic"),
            (Section::Timezone, "Etc/UTC"),
        ]));
        assert_eq!(sections.get(Section::Kernel).trim(), "6.8.0-90-generic");
        assert_eq!(sections.status(Section::Kernel), Some(0));
        assert_eq!(sections.get(Section::Timezone).trim(), "Etc/UTC");
        assert_eq!(sections.get(Section::Os), "");
        assert_eq!(sections.status(Section::Os), None);
    }

    #[test]
    fn sections_parse_exit_status() {
        let mut output = fake_section(Section::Ufw, "ufw: command not found", Some(127));
        output.push_str(&fake_section(Section::Timezone, "cut off", None));
        let sections = Sections::parse(&output);
        assert_eq!(sections.status(Section::Ufw), Some(127));
        assert_eq!(sections.get(Section::Ufw).trim(), "ufw: command not found");
        assert_eq!(sections.status(Section::Timezone), None);
    }

    #[test]
    fn normalizer_strips_runner_key() {
        let output = runner_output(
            "runner-a",
            "00000000-0000-0000-0000-00000000000a",
            ["nvme0n1p3[0]", "nvme1n1p3[1]"],
            "6.8.0-90-generic",
        );
        assert!(output.contains(KEY));
        let sections = Sections::parse(&output);
        let snapshot = Snapshot::new(&sections);
        for text in [
            format!("{sections:?}"),
            format!("{snapshot:?}"),
            snapshot.to_string(),
        ] {
            assert!(!text.contains(KEY), "{text}");
            assert!(!text.contains(RUNNER_KEY), "{text}");
        }
        assert!(
            snapshot
                .to_string()
                .contains("Environment=BENCHER_RUNNER=<runner>\n")
        );
    }

    #[test]
    fn runner_unit_command_reads_only_allowlisted_lines() {
        let dir = tempfile::tempdir().unwrap();
        let unit = camino::Utf8Path::from_path(dir.path())
            .unwrap()
            .join("bencher-runner.service");
        std::fs::write(
            &unit,
            format!(
                "[Service]\nExecStart=/usr/local/bin/runner up --key {KEY}\nExecStart=/usr/local/bin/runner up\nPrivateTmp=yes\n"
            ),
        )
        .unwrap();
        std::fs::create_dir(format!("{unit}.d")).unwrap();
        std::fs::write(
            format!("{unit}.d/credentials.conf"),
            format!(
                "Environment=BENCHER_RUNNER=runner-a\nEnvironmentFile=/etc/bencher-runner/key.env\nEnvironment=BENCHER_RUNNER_KEY={KEY}\nEnvironment=\"BENCHER_RUNNER_KEY={KEY}\"\n"
            ),
        )
        .unwrap();
        let output = Command::new("sh")
            .arg("-c")
            .arg(runner_unit_command(unit.as_str()))
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!(
                "{unit}:[Service]\n{unit}:ExecStart=/usr/local/bin/runner up\n{unit}.d/credentials.conf:Environment=BENCHER_RUNNER=runner-a\n{unit}.d/credentials.conf:EnvironmentFile=/etc/bencher-runner/key.env\n"
            )
        );
    }

    #[test]
    fn normalize_cmdline_root() {
        assert_eq!(
            normalize_cmdline(
                "BOOT_IMAGE=/vmlinuz root=UUID=00000000-0000-0000-0000-00000000000a ro"
            ),
            "BOOT_IMAGE=/vmlinuz root=UUID=<root> ro"
        );
    }

    #[test]
    fn normalize_raid_member_order() {
        assert_eq!(
            normalize_raid_array("md2 : active raid1 nvme1n1p3[1] nvme0n1p3[0]"),
            Some("md2 : active raid1 nvme0n1p3 nvme1n1p3".to_owned())
        );
        assert_eq!(
            normalize_raid_array("md2 : active raid1 nvme0n1p3[0] nvme1n1p3[1](F)"),
            Some("md2 : active raid1 nvme0n1p3 nvme1n1p3(F)".to_owned())
        );
        assert_eq!(normalize_raid_array("Personalities : [raid1]"), None);
    }

    #[test]
    fn normalize_raid_keeps_arrays_sizes_and_spares() {
        let mdstat = "\
Personalities : [raid1]
md2 : active raid1 nvme2n1p3[2](S) nvme1n1p3[1] nvme0n1p3[0]
      9000 blocks super 1.2 [2/2] [UU]
      [=>...................]  resync =  8.5% (765/9000) finish=37.2min speed=1000K/sec
      bitmap: 1/1 pages [4KB], 65536KB chunk

unused devices: <none>";
        assert_eq!(
            Section::Raid.normalize(mdstat),
            [
                "md2 : active raid1 nvme0n1p3 nvme1n1p3 nvme2n1p3(S)",
                "md2: 9000 blocks super 1.2 [2/2] [UU]",
            ]
        );
    }

    #[test]
    fn normalize_runner_name_value() {
        assert_eq!(
            normalize_runner_name("Environment=BENCHER_RUNNER=runner-a"),
            "Environment=BENCHER_RUNNER=<runner>"
        );
        assert_eq!(
            normalize_runner_name("Environment=\"BENCHER_RUNNER=runner-a\""),
            "Environment=\"BENCHER_RUNNER=<runner>\""
        );
        assert_eq!(
            normalize_runner_name("Environment=BENCHER_HOST=https://api.example.com/"),
            "Environment=BENCHER_HOST=https://api.example.com/"
        );
    }

    #[test]
    fn diff_ignores_values_that_must_differ() {
        let runner = snapshot(&runner_output(
            "runner-a",
            "00000000-0000-0000-0000-00000000000a",
            ["nvme0n1p3[0]", "nvme1n1p3[1]"],
            "6.8.0-90-generic",
        ));
        let reference = snapshot(&runner_output(
            "runner-b",
            "00000000-0000-0000-0000-00000000000b",
            ["nvme1n1p3[1]", "nvme0n1p3[0]"],
            "6.8.0-90-generic",
        ));
        assert_eq!(runner.diff(&reference), Vec::new());
    }

    #[test]
    fn diff_reports_differing_lines_by_section() {
        let runner = snapshot(&runner_output(
            "runner-a",
            "00000000-0000-0000-0000-00000000000a",
            ["nvme0n1p3[0]", "nvme1n1p3[1]"],
            "6.8.0-90-generic",
        ));
        let reference = snapshot(&runner_output(
            "runner-b",
            "00000000-0000-0000-0000-00000000000b",
            ["nvme0n1p3[0]", "nvme1n1p3[1]"],
            "6.8.0-85-generic",
        ));
        let diff = runner.diff(&reference);
        assert_eq!(
            diff,
            [
                SectionDiff {
                    section: Section::Kernel,
                    reference: vec!["6.8.0-85-generic"],
                    runner: vec!["6.8.0-90-generic"],
                    order_differs: false,
                },
                SectionDiff {
                    section: Section::Cmdline,
                    reference: vec![
                        "BOOT_IMAGE=/vmlinuz-6.8.0-85-generic root=UUID=<root> ro isolcpus=1-5"
                    ],
                    runner: vec![
                        "BOOT_IMAGE=/vmlinuz-6.8.0-90-generic root=UUID=<root> ro isolcpus=1-5"
                    ],
                    order_differs: false,
                },
            ]
        );
        assert_eq!(
            diff.first().unwrap().to_string(),
            "[kernel]\n- 6.8.0-85-generic\n+ 6.8.0-90-generic\n"
        );
    }

    #[test]
    fn diff_ignores_package_order() {
        let runner = snapshot(&fake_output(&[(
            Section::Packages,
            "curl 8.5.0\nzlib1g 1:1.3",
        )]));
        let reference = snapshot(&fake_output(&[(
            Section::Packages,
            "zlib1g 1:1.3\ncurl 8.5.0",
        )]));
        assert_eq!(runner.diff(&reference), Vec::new());
    }

    #[test]
    fn diff_leaves_out_health_sections() {
        let runner = snapshot(&fake_output(&[(Section::RunnerService, "active")]));
        let reference = snapshot(&fake_output(&[(Section::RunnerService, "inactive")]));
        assert_eq!(runner.diff(&reference), Vec::new());
    }

    #[test]
    fn diff_reports_reordered_lines() {
        let runner = snapshot(&fake_output(&[(Section::Grub, "a.cfg:X=1\nb.cfg:X=2")]));
        let reference = snapshot(&fake_output(&[(Section::Grub, "b.cfg:X=2\na.cfg:X=1")]));
        let diff = runner.diff(&reference);
        assert_eq!(
            diff,
            [SectionDiff {
                section: Section::Grub,
                reference: Vec::new(),
                runner: Vec::new(),
                order_differs: true,
            }]
        );
        assert_eq!(
            diff.first().unwrap().to_string(),
            "[grub]\n~ order differs\n"
        );
    }

    #[test]
    fn diff_reports_repeated_lines() {
        let runner = snapshot(&fake_output(&[(Section::Apt, "a\nb\nb")]));
        let reference = snapshot(&fake_output(&[(Section::Apt, "a\nb")]));
        assert_eq!(runner.diff(&reference).len(), 1);
    }

    #[test]
    fn diff_reports_exit_status() {
        let runner = snapshot(&fake_section(
            Section::Ufw,
            "ufw: command not found",
            Some(127),
        ));
        let reference = snapshot(&fake_section(
            Section::Ufw,
            "ufw: command not found",
            Some(0),
        ));
        assert_eq!(
            runner.diff(&reference),
            [SectionDiff {
                section: Section::Ufw,
                reference: Vec::new(),
                runner: vec!["exit status: 127"],
                order_differs: false,
            }]
        );
    }

    #[test]
    fn snapshot_size() {
        let snapshot = snapshot(&fake_output(&[
            (Section::Kernel, "6.8.0-90-generic"),
            (Section::Packages, "curl 8.5.0\nzlib1g 1:1.3"),
        ]));
        let (sections, lines) = snapshot.size();
        assert_eq!(sections, 17);
        // 3 lines of output, plus a missing exit status for each of the other 15 sections
        assert_eq!(lines, 18);
    }

    fn mdstat(arrays: &[&str]) -> String {
        format!(
            "Personalities : [raid1]\n{}\nunused devices: <none>",
            arrays.join("\n")
        )
    }

    const MD0: &str = "\
md0 : active raid1 nvme1n1p1[1] nvme0n1p1[0]
      4000 blocks super 1.2 [2/2] [UU]
";
    const MD1: &str = "\
md1 : active raid1 nvme0n1p2[0] nvme1n1p2[1]
      1000 blocks super 1.2 [2/2] [UU]
";
    const MD2: &str = "\
md2 : active raid1 nvme0n1p3[0] nvme1n1p3[1]
      9000 blocks super 1.2 [2/2] [UU]
      bitmap: 3/8 pages [12KB], 65536KB chunk
";

    #[test]
    fn diff_ignores_raid_array_order() {
        let runner = snapshot(&fake_output(&[(Section::Raid, &mdstat(&[MD0, MD1, MD2]))]));
        let reference = snapshot(&fake_output(&[(Section::Raid, &mdstat(&[MD0, MD2, MD1]))]));
        assert_eq!(runner.diff(&reference), Vec::new());
    }

    #[test]
    fn diff_reports_raid_content_inside_an_array() {
        let degraded = MD1.replace("[2/2] [UU]", "[2/1] [U_]");
        let runner = snapshot(&fake_output(&[(Section::Raid, &mdstat(&[MD0, MD1, MD2]))]));
        let reference = snapshot(&fake_output(&[(
            Section::Raid,
            &mdstat(&[MD0, MD2, &degraded]),
        )]));
        assert_eq!(
            runner.diff(&reference),
            [SectionDiff {
                section: Section::Raid,
                reference: vec!["md1: 1000 blocks super 1.2 [2/1] [U_]"],
                runner: vec!["md1: 1000 blocks super 1.2 [2/2] [UU]"],
                order_differs: false,
            }]
        );
    }

    #[test]
    fn normalize_raid_sorts_arrays_numerically() {
        let md9 = "md9 : active raid1 sdb1[1] sda1[0]\n      10 blocks super 1.2 [2/2] [UU]";
        let md10 = "md10 : active raid1 sdb2[1] sda2[0]\n      20 blocks super 1.2 [2/2] [UU]";
        assert_eq!(
            Section::Raid.normalize(&mdstat(&[md10, md9])),
            [
                "md9 : active raid1 sda1 sdb1",
                "md9: 10 blocks super 1.2 [2/2] [UU]",
                "md10 : active raid1 sda2 sdb2",
                "md10: 20 blocks super 1.2 [2/2] [UU]",
            ]
        );
    }

    #[test]
    fn diff_reports_raid_sizes_swapped_between_arrays() {
        let swapped_md1 = MD1.replace("1000 blocks", "9000 blocks");
        let swapped_md2 = MD2.replace("9000 blocks", "1000 blocks");
        let runner = snapshot(&fake_output(&[(Section::Raid, &mdstat(&[MD0, MD1, MD2]))]));
        let reference = snapshot(&fake_output(&[(
            Section::Raid,
            &mdstat(&[MD0, &swapped_md1, &swapped_md2]),
        )]));
        assert_eq!(
            runner.diff(&reference),
            [SectionDiff {
                section: Section::Raid,
                reference: vec![
                    "md1: 9000 blocks super 1.2 [2/2] [UU]",
                    "md2: 1000 blocks super 1.2 [2/2] [UU]",
                ],
                runner: vec![
                    "md1: 1000 blocks super 1.2 [2/2] [UU]",
                    "md2: 9000 blocks super 1.2 [2/2] [UU]",
                ],
                order_differs: false,
            }]
        );
    }
}
