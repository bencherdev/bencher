use std::collections::BTreeSet;
use std::fmt;

use super::snapshot::{Section, Sections};
use crate::task::apt::autoremovable_packages;
use crate::task::isolate::is_isolated;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Check {
    Content,
    Commands,
    RunnerService,
    RebootRequired,
    Raid,
    Autoremove,
    Isolation,
}

impl fmt::Display for Check {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Content => "every section has content",
            Self::Commands => "every audit command succeeded",
            Self::RunnerService => "runner service is active",
            Self::RebootRequired => "no reboot is required",
            Self::Raid => "RAID arrays are in sync",
            Self::Autoremove => "no packages are auto-removable",
            Self::Isolation => "CPU isolation boot args are set",
        })
    }
}

/// The result of one health check; `problem` is `None` when it passes.
#[derive(Debug, PartialEq, Eq)]
pub struct Finding {
    pub check: Check,
    pub problem: Option<String>,
}

impl Finding {
    pub fn failed(&self) -> bool {
        self.problem.is_some()
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { check, problem } = self;
        match problem {
            Some(problem) => write!(f, "FAIL  {check}: {problem}"),
            None => write!(f, "ok    {check}"),
        }
    }
}

pub fn check(sections: &Sections) -> Vec<Finding> {
    [
        (Check::Content, content(sections)),
        (Check::Commands, commands(sections)),
        (
            Check::RunnerService,
            runner_service(sections.get(Section::RunnerService)),
        ),
        (
            Check::RebootRequired,
            reboot_required(sections.get(Section::RebootRequired)),
        ),
        (Check::Raid, raid(sections.get(Section::Raid))),
        (
            Check::Autoremove,
            autoremove(sections.get(Section::Autoremove)),
        ),
        (Check::Isolation, isolation(sections.get(Section::Cmdline))),
    ]
    .into_iter()
    .map(|(check, problem)| Finding { check, problem })
    .collect()
}

/// A clean audit must never come from empty data.
fn content(sections: &Sections) -> Option<String> {
    let empty: Vec<String> = Section::ALL
        .into_iter()
        .filter(|&section| section.normalize(sections.get(section)).is_empty())
        .map(|section| section.to_string())
        .collect();
    (!empty.is_empty()).then(|| format!("no output from {}", empty.join(", ")))
}

fn commands(sections: &Sections) -> Option<String> {
    let failed: Vec<String> = Section::ALL
        .into_iter()
        .filter(|section| section.expects_success())
        .filter_map(|section| match sections.status(section) {
            Some(0) => None,
            Some(status) => Some(format!("{section} (exit {status})")),
            None => Some(format!("{section} (no exit status)")),
        })
        .collect();
    (!failed.is_empty()).then(|| failed.join(", "))
}

fn runner_service(state: &str) -> Option<String> {
    match state.trim() {
        "active" => None,
        "" => Some("bencher-runner state is unknown".to_owned()),
        state => Some(format!("bencher-runner is {state}")),
    }
}

fn reboot_required(output: &str) -> Option<String> {
    let mut lines = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty());
    match lines.next() {
        Some("none") => None,
        Some("required") => {
            let packages: BTreeSet<&str> = lines.collect();
            Some(if packages.is_empty() {
                "a reboot is pending".to_owned()
            } else {
                format!(
                    "a reboot is pending for {}",
                    packages.into_iter().collect::<Vec<_>>().join(" ")
                )
            })
        },
        _ => Some(format!("unexpected output: {}", output.trim())),
    }
}

fn raid(mdstat: &str) -> Option<String> {
    if !mdstat.contains("Personalities") {
        return Some(format!("cannot read /proc/mdstat: {}", mdstat.trim()));
    }
    let mut problems = Vec::new();
    let mut array = "";
    for line in mdstat.lines().map(str::trim) {
        if let Some((name, members)) = line.split_once(" : ")
            && name.starts_with("md")
        {
            array = name;
            if members.split_whitespace().next() == Some("inactive") {
                problems.push(format!("{array} is inactive"));
            }
            if members.contains("(F)") {
                problems.push(format!("{array} has a failed member"));
            }
        } else if let Some(start) = line.find("resync").or_else(|| line.find("recovery")) {
            let (_, progress) = line.split_at(start);
            let progress = progress.split_whitespace().collect::<Vec<_>>().join(" ");
            problems.push(format!("{array} {progress}"));
        } else if line.split_whitespace().any(is_degraded) {
            problems.push(format!("{array} is degraded"));
        }
    }
    (!problems.is_empty()).then(|| problems.join("; "))
}

/// A member status field such as `[U_]`, where `_` is a missing member.
fn is_degraded(field: &str) -> bool {
    field
        .strip_prefix('[')
        .and_then(|field| field.strip_suffix(']'))
        .is_some_and(|status| status.contains('_') && status.chars().all(|c| c == 'U' || c == '_'))
}

fn autoremove(simulation: &str) -> Option<String> {
    if !simulation.contains(" to remove and ") {
        return Some(format!(
            "could not simulate autoremove: {}",
            simulation.trim()
        ));
    }
    let packages = autoremovable_packages(simulation);
    (!packages.is_empty()).then(|| packages.join(" "))
}

fn isolation(cmdline: &str) -> Option<String> {
    (!is_isolated(cmdline)).then(|| "the kernel cmdline has no isolcpus= or nohz_full=".to_owned())
}

#[cfg(test)]
mod tests {
    use super::super::snapshot::{fake_output, fake_section};
    use super::*;

    const MDSTAT_SYNCED: &str = "\
Personalities : [raid1]
md0 : active raid1 nvme1n1p1[1] nvme0n1p1[0]
      1000 blocks super 1.2 [2/2] [UU]

md2 : active raid1 nvme0n1p3[0] nvme1n1p3[1]
      9000 blocks super 1.2 [2/2] [UU]
      bitmap: 1/1 pages [4KB], 65536KB chunk

unused devices: <none>";
    const AUTOREMOVE_CLEAN: &str = "\
NOTE: This is only a simulation!
0 upgraded, 0 newly installed, 0 to remove and 0 not upgraded.";

    fn healthy() -> Vec<(Section, &'static str)> {
        Section::ALL
            .into_iter()
            .map(|section| {
                let text = match section {
                    Section::RunnerService => "active",
                    Section::RebootRequired => "none",
                    Section::Raid => MDSTAT_SYNCED,
                    Section::Autoremove => AUTOREMOVE_CLEAN,
                    Section::Cmdline => {
                        "BOOT_IMAGE=/vmlinuz ro isolcpus=1-5 nohz_full=1-5 rcu_nocbs=1-5"
                    },
                    Section::Os
                    | Section::Kernel
                    | Section::KernelPackages
                    | Section::Hardware
                    | Section::Smt
                    | Section::RunnerBinary
                    | Section::RunnerUnit
                    | Section::Sshd
                    | Section::Apt
                    | Section::Grub
                    | Section::Ufw
                    | Section::EnabledUnits
                    | Section::Timezone
                    | Section::Packages
                    | Section::ManualPackages => "output",
                };
                (section, text)
            })
            .collect()
    }

    /// The problems reported after replacing one section of a healthy runner.
    fn problems(section: Section, text: &str) -> Vec<(Check, String)> {
        failures(section, text, Some(0))
    }

    /// The problems reported after replacing one section and its exit status.
    fn failures(section: Section, text: &str, status: Option<i32>) -> Vec<(Check, String)> {
        let output: String = healthy()
            .into_iter()
            .map(|(healthy_section, healthy_text)| {
                if healthy_section == section {
                    fake_section(section, text, status)
                } else {
                    fake_section(healthy_section, healthy_text, Some(0))
                }
            })
            .collect();
        check(&Sections::parse(&output))
            .into_iter()
            .filter_map(|Finding { check, problem }| problem.map(|problem| (check, problem)))
            .collect()
    }

    #[test]
    fn healthy_runner_passes() {
        let findings = check(&Sections::parse(&fake_output(&healthy())));
        assert_eq!(findings.len(), 7);
        assert!(
            findings.iter().all(|finding| !finding.failed()),
            "{findings:?}"
        );
    }

    #[test]
    fn runner_service_inactive() {
        assert_eq!(
            problems(Section::RunnerService, "inactive"),
            [(
                Check::RunnerService,
                "bencher-runner is inactive".to_owned()
            )]
        );
    }

    #[test]
    fn reboot_required_lists_packages() {
        assert_eq!(
            problems(
                Section::RebootRequired,
                "required\nlinux-image-6.8.0-90-generic\nlinux-base\nlinux-base"
            ),
            [(
                Check::RebootRequired,
                "a reboot is pending for linux-base linux-image-6.8.0-90-generic".to_owned()
            )]
        );
        assert_eq!(
            problems(Section::RebootRequired, ""),
            [
                (Check::Content, "no output from reboot required".to_owned()),
                (Check::RebootRequired, "unexpected output: ".to_owned())
            ]
        );
    }

    #[test]
    fn raid_resync_in_progress() {
        let mdstat = "\
Personalities : [raid1]
md2 : active raid1 nvme0n1p3[0] nvme1n1p3[1]
      9000 blocks super 1.2 [2/2] [UU]
      [=>...................]  resync =  8.5% (765/9000) finish=37.2min speed=1000K/sec
      bitmap: 1/1 pages [4KB], 65536KB chunk

unused devices: <none>";
        assert_eq!(
            problems(Section::Raid, mdstat),
            [(
                Check::Raid,
                "md2 resync = 8.5% (765/9000) finish=37.2min speed=1000K/sec".to_owned()
            )]
        );
    }

    #[test]
    fn raid_recovery_on_degraded_array() {
        let mdstat = "\
Personalities : [raid1]
md2 : active raid1 nvme1n1p3[2] nvme0n1p3[0]
      9000 blocks super 1.2 [2/1] [U_]
      [===>.................]  recovery = 17.0% (1530/9000) finish=20.1min speed=1000K/sec

unused devices: <none>";
        assert_eq!(
            problems(Section::Raid, mdstat),
            [(
                Check::Raid,
                "md2 is degraded; md2 recovery = 17.0% (1530/9000) finish=20.1min speed=1000K/sec"
                    .to_owned()
            )]
        );
    }

    #[test]
    fn raid_failed_member() {
        let mdstat = "\
Personalities : [raid1]
md2 : active raid1 nvme1n1p3[1](F) nvme0n1p3[0]
      9000 blocks super 1.2 [2/1] [U_]

unused devices: <none>";
        assert_eq!(
            problems(Section::Raid, mdstat),
            [(
                Check::Raid,
                "md2 has a failed member; md2 is degraded".to_owned()
            )]
        );
    }

    #[test]
    fn raid_unreadable() {
        assert_eq!(
            failures(
                Section::Raid,
                "cat: /proc/mdstat: No such file or directory",
                Some(1)
            ),
            [
                (Check::Content, "no output from raid".to_owned()),
                (Check::Commands, "raid (exit 1)".to_owned()),
                (
                    Check::Raid,
                    "cannot read /proc/mdstat: cat: /proc/mdstat: No such file or directory"
                        .to_owned()
                )
            ]
        );
    }

    #[test]
    fn raid_inactive() {
        let mdstat = "\
Personalities : [raid1]
md2 : inactive nvme1n1p3[1](S) nvme0n1p3[0](S)
      9000 blocks super 1.2

unused devices: <none>";
        assert_eq!(
            problems(Section::Raid, mdstat),
            [(Check::Raid, "md2 is inactive".to_owned())]
        );
    }

    #[test]
    fn empty_section_fails() {
        assert_eq!(
            problems(Section::Timezone, ""),
            [(Check::Content, "no output from timezone".to_owned())]
        );
    }

    #[test]
    fn failed_command_fails() {
        assert_eq!(
            failures(Section::Ufw, "ufw: command not found", Some(127)),
            [(Check::Commands, "ufw (exit 127)".to_owned())]
        );
        assert_eq!(
            failures(Section::Timezone, "Etc/UTC", None),
            [(Check::Commands, "timezone (no exit status)".to_owned())]
        );
    }

    #[test]
    fn inactive_runner_service_is_not_a_command_failure() {
        assert_eq!(
            failures(Section::RunnerService, "inactive", Some(3)),
            [(
                Check::RunnerService,
                "bencher-runner is inactive".to_owned()
            )]
        );
    }

    #[test]
    fn autoremovable_packages_fail() {
        let simulation = "\
0 upgraded, 0 newly installed, 1 to remove and 0 not upgraded.
Remv bpftrace [0.20.2-1ubuntu4]";
        assert_eq!(
            problems(Section::Autoremove, simulation),
            [(Check::Autoremove, "bpftrace".to_owned())]
        );
        assert_eq!(
            problems(Section::Autoremove, "E: broken"),
            [(
                Check::Autoremove,
                "could not simulate autoremove: E: broken".to_owned()
            )]
        );
    }

    #[test]
    fn isolation_args_missing() {
        assert_eq!(
            problems(Section::Cmdline, "BOOT_IMAGE=/vmlinuz ro"),
            [(
                Check::Isolation,
                "the kernel cmdline has no isolcpus= or nohz_full=".to_owned()
            )]
        );
    }
}
