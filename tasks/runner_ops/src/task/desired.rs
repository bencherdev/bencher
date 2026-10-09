use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use bencher_json::{Sha256, UpdateChannel};
use bencher_runner::maintenance::{JOB_LOCK, MARKER, RUN_DIR};

use super::framed::{self, Output};
use super::isolate::{format_cpu_list, parse_cpu_list};
use super::start::KEY_FILE;
use crate::parser::scrub_day::ScrubDay;

pub const SSH_HARDENING_PATH: &str = "/etc/ssh/sshd_config.d/hardening.conf";
pub const UNATTENDED_UPGRADES_PATH: &str = "/etc/apt/apt.conf.d/50unattended-upgrades-local";
pub const AUTO_UPGRADES_PATH: &str = "/etc/apt/apt.conf.d/20auto-upgrades";
pub const KERNEL_PIN_PATH: &str = "/etc/apt/preferences.d/bencher-kernels.pref";
const SCRUB_TIMER: &str = "mdcheck_start.timer";
// The `zz-` prefix sorts it after the installer's drop-in, which sets a random day.
const SCRUB_DROP_IN: &str = "/etc/systemd/system/mdcheck_start.timer.d/zz-bencher-scrub.conf";
// The persistent timer's last trigger, which systemd reads when the timer starts.
const SCRUB_STAMP: &str = "/var/lib/systemd/timers/stamp-mdcheck_start.timer";
pub const BENCHER_CGROUP: &str = "/sys/fs/cgroup/bencher";
const ISOLATED_CPUS: &str = "/sys/devices/system/cpu/isolated";
/// The controllers the runner enables for its `bencher/` cgroup.
pub const RUNNER_CONTROLLERS: [&str; 3] = ["cpuset", "memory", "pids"];
const STATE_DIR: &str = "/var/lib/bencher-runner";
const STATE_DIR_STAT: &str = "700 root:root directory";
const RUNNER_BINARY: &str = "/usr/local/bin/runner";
const ROOT: &str = "root";
// Units with no use on a runner that could wake during a Job; each socket, path, or timer comes before any service it starts.
const MASKED_UNITS: [&str; 34] = [
    "thermald.service",
    "man-db.timer",
    "motd-news.timer",
    "update-notifier-download.timer",
    "update-notifier-motd.timer",
    "e2scrub_all.timer",
    "sysstat-collect.timer",
    "sysstat-summary.timer",
    "sysstat.service",
    "ua-timer.timer",
    "ua-reboot-cmds.service",
    "ubuntu-advantage.service",
    "apport-autoreport.path",
    "apport-autoreport.timer",
    "apport-forward.socket",
    "apport.service",
    "multipathd.socket",
    "multipathd.service",
    "iscsid.socket",
    "open-iscsi.service",
    "open-vm-tools.service",
    "vgauth.service",
    "lxd-installer.socket",
    "gpu-manager.service",
    "pollinate.service",
    "snapd.socket",
    "snapd.service",
    "snapd.seeded.service",
    "snapd.snap-repair.timer",
    "snapd.apparmor.service",
    "snapd.autoimport.service",
    "snapd.core-fixup.service",
    "snapd.recovery-chooser-trigger.service",
    "snapd.system-shutdown.service",
];
/// Has a maintenance unit take turns with the runner's Jobs; `zz-` sorts it after any other drop-in.
const QUIET_DROP_IN: &str = "zz-bencher-quiet.conf";
/// Each maintenance unit and its own command, which the drop-in runs under the runner's job lock.
const QUIET_UNITS: [(&str, &str); 5] = [
    ("apt-daily.service", "/usr/lib/apt/apt.systemd.daily update"),
    (
        "apt-daily-upgrade.service",
        "/usr/lib/apt/apt.systemd.daily install",
    ),
    (
        "fstrim.service",
        "/sbin/fstrim --listed-in /etc/fstab:/proc/self/mountinfo --verbose --quiet-unsupported",
    ),
    (
        "mdcheck_start.service",
        "/usr/share/mdadm/mdcheck --duration ${MDADM_CHECK_DURATION}",
    ),
    (
        "mdcheck_continue.service",
        "/usr/share/mdadm/mdcheck --continue --duration ${MDADM_CHECK_DURATION}",
    ),
];
const ABSENT: &str = "absent";
const FRAME: &str = "desired ";

const SSH_HARDENING_CONF: &str = "\
PasswordAuthentication no
ChallengeResponseAuthentication no
KbdInteractiveAuthentication no
X11Forwarding no
PermitRootLogin prohibit-password";

const UNATTENDED_UPGRADES_CONF: &str = "\
Unattended-Upgrade::AutoFixInterruptedDpkg \"true\";
Unattended-Upgrade::Remove-Unused-Kernel-Packages \"true\";
Unattended-Upgrade::Remove-Unused-Dependencies \"true\";";

const AUTO_UPGRADES_CONF: &str = "\
APT::Periodic::Update-Package-Lists \"1\";
APT::Periodic::Unattended-Upgrade \"1\";";

// A new kernel reaches a runner only from the security pocket.
const KERNEL_PIN_CONF: &str = "\
Package: linux-*
Pin: release a=noble-updates
Pin-Priority: -1";

/// The state a runner host must be in, one row per setting: `host` writes the rows it sets, and `audit` checks every row.
#[derive(Debug)]
pub struct Desired(Vec<Row>);

impl Desired {
    pub fn new(update_channel: UpdateChannel, scrub_day: Option<ScrubDay>) -> Self {
        let mut rows = vec![
            Row::File(File::root(
                SSH_HARDENING_PATH,
                0o644,
                Content::Text(SSH_HARDENING_CONF.to_owned()),
                Some("systemctl reload ssh"),
            )),
            Row::File(File::root(
                UNATTENDED_UPGRADES_PATH,
                0o644,
                Content::Text(UNATTENDED_UPGRADES_CONF.to_owned()),
                None,
            )),
            Row::File(File::root(
                AUTO_UPGRADES_PATH,
                0o644,
                Content::Text(AUTO_UPGRADES_CONF.to_owned()),
                None,
            )),
            Row::File(File::root(
                KERNEL_PIN_PATH,
                0o644,
                Content::Text(KERNEL_PIN_CONF.to_owned()),
                None,
            )),
            Row::File(File::root(KEY_FILE, 0o600, Content::Secret, None)),
            // Before the masks, whose daemon reloads would re-arm the timer on its old calendar.
            Row::Scrub(scrub_day),
        ];
        for (unit, command) in QUIET_UNITS {
            rows.extend([
                Row::File(File::root(
                    &quiet_drop_in_path(unit),
                    0o644,
                    Content::Text(quiet_drop_in(command)),
                    Some("systemctl daemon-reload"),
                )),
                Row::Vendor { unit, command },
            ]);
        }
        rows.extend(MASKED_UNITS.into_iter().map(Row::Masked));
        rows.extend([
            Row::Cgroup(Cgroup::Controllers),
            Row::Cgroup(Cgroup::Cpus),
            Row::Cgroup(Cgroup::Partition),
            Row::StateDir,
            Row::Binary(update_channel),
        ]);
        Self(rows)
    }

    /// Each row's frame name and read-only command.
    pub fn frames(&self) -> impl Iterator<Item = (String, String)> {
        self.0
            .iter()
            .enumerate()
            .map(|(index, row)| (frame(index), row.read()))
    }

    fn script(&self) -> String {
        framed::script(self.frames())
    }

    /// Check every row against its frame, fetching a channel's published checksum only to compare an installed binary.
    pub fn check(
        &self,
        frames: &BTreeMap<String, Output>,
        published: &mut dyn FnMut(UpdateChannel) -> anyhow::Result<Sha256>,
    ) -> Vec<Finding> {
        self.0
            .iter()
            .enumerate()
            .map(|(index, row)| Finding {
                name: row.name(),
                set_by: row.set_by(),
                problem: frames.get(&frame(index)).map_or_else(
                    || Some("was not read".to_owned()),
                    |output| row.problem(output, published),
                ),
            })
            .collect()
    }

    /// Read every row, write each differing row `host` sets, then read every row again.
    pub fn apply<R, W>(
        &self,
        mut read: R,
        mut write: W,
        published: &mut dyn FnMut(UpdateChannel) -> anyhow::Result<Sha256>,
    ) -> anyhow::Result<Applied>
    where
        R: FnMut(&str) -> anyhow::Result<String>,
        W: FnMut(&str) -> anyhow::Result<String>,
    {
        let script = self.script();
        let before = self.check(&framed::parse(&read(&script)?), published);
        let (writes, reloads) = self.writes(&before);
        for command in writes.iter().map(String::as_str).chain(reloads) {
            write(command)?;
        }
        Ok(Applied {
            written: writes.len(),
            findings: self.check(&framed::parse(&read(&script)?), published),
        })
    }

    /// The command that writes each differing row `host` sets, and each reload those writes need, once.
    fn writes(&self, findings: &[Finding]) -> (Vec<String>, Vec<&'static str>) {
        let mut writes = Vec::new();
        let mut reloads = Vec::new();
        for (row, finding) in self.0.iter().zip(findings) {
            if finding.failed()
                && let Some(write) = row.write()
            {
                writes.push(write);
                if let Some(reload) = row.reload()
                    && !reloads.contains(&reload)
                {
                    reloads.push(reload);
                }
            }
        }
        (writes, reloads)
    }
}

/// The rows `apply` wrote, and every row as read after its writes.
#[derive(Debug)]
pub struct Applied {
    pub written: usize,
    pub findings: Vec<Finding>,
}

/// Print one runner's desired state and count the rows that differ.
pub fn report(label: &str, findings: &[Finding]) -> usize {
    println!("Desired state of {label}:");
    for finding in findings {
        println!("  {finding}");
    }
    findings.iter().filter(|finding| finding.failed()).count()
}

/// One row of a runner's desired state; `problem` is `None` when the runner matches it.
#[derive(Debug, PartialEq, Eq)]
pub struct Finding {
    name: String,
    set_by: SetBy,
    problem: Option<String>,
}

impl Finding {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn failed(&self) -> bool {
        self.problem.is_some()
    }

    /// Whether the row differs although `host` sets it.
    pub fn host_unset(&self) -> bool {
        self.failed() && self.set_by == SetBy::Host
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self {
            name,
            set_by,
            problem,
        } = self;
        match problem {
            Some(problem) => write!(f, "FAIL  {name}: {problem} (set by {set_by})"),
            None => write!(f, "ok    {name}"),
        }
    }
}

/// What sets a row, so a difference names its fix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SetBy {
    Host,
    Start,
    Deploy,
    Runner,
    /// The distribution's own unit, whose command a drop-in restates.
    Vendor,
}

impl fmt::Display for SetBy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Host => "`cargo ops host`",
            Self::Start => "`cargo ops start`",
            Self::Deploy => "`cargo ops deploy`",
            Self::Runner => "the runner",
            Self::Vendor => {
                "the unit's package or another drop-in; restate the package's command in the drop-in, and set it in no other"
            },
        })
    }
}

#[derive(Debug)]
enum Row {
    File(File),
    /// A unit masked and stopped, so it never runs beside a Job.
    Masked(&'static str),
    /// The monthly RAID scrub, on the runner's scrub day from runners.json with no random delay.
    Scrub(Option<ScrubDay>),
    /// The runner's own cgroup, which it sets up at startup.
    Cgroup(Cgroup),
    StateDir,
    /// The installed runner binary, checked against the checksum its channel's release publishes.
    Binary(UpdateChannel),
    /// A maintenance unit's own command, which its drop-in restates, so a package update that changes it shows.
    Vendor {
        unit: &'static str,
        command: &'static str,
    },
}

#[derive(Debug)]
struct File {
    path: String,
    mode: u32,
    user: String,
    group: String,
    content: Content,
    /// Run once after the file is written, for the change to take effect.
    reload: Option<&'static str>,
}

#[derive(Debug)]
enum Content {
    Text(String),
    /// Never read nor written here, so only its mode and owner are checked.
    Secret,
}

#[derive(Debug, Clone, Copy)]
enum Cgroup {
    Controllers,
    /// Its effective CPUs, which must be exactly the CPUs the kernel isolates.
    Cpus,
    Partition,
}

impl Row {
    fn name(&self) -> String {
        match self {
            Self::File(file) => file.path.clone(),
            Self::Masked(unit) => format!("{unit} masked"),
            Self::Scrub(_) => format!("{SCRUB_TIMER} on the scrub day"),
            Self::Cgroup(Cgroup::Controllers) => format!("{BENCHER_CGROUP} controllers"),
            Self::Cgroup(Cgroup::Cpus) => format!("{BENCHER_CGROUP} CPUs"),
            Self::Cgroup(Cgroup::Partition) => format!("{BENCHER_CGROUP} partition"),
            Self::StateDir => STATE_DIR.to_owned(),
            Self::Binary(channel) => format!("{RUNNER_BINARY} ({channel})"),
            Self::Vendor { unit, .. } => format!("{unit} command"),
        }
    }

    fn set_by(&self) -> SetBy {
        match self {
            Self::File(File {
                content: Content::Secret,
                ..
            }) => SetBy::Start,
            Self::File(_) | Self::Masked(_) | Self::Scrub(_) => SetBy::Host,
            Self::Cgroup(_) | Self::StateDir => SetBy::Runner,
            Self::Binary(_) => SetBy::Deploy,
            Self::Vendor { .. } => SetBy::Vendor,
        }
    }

    /// A read-only command that prints the row's current value.
    fn read(&self) -> String {
        match self {
            Self::File(file) => file.read(),
            Self::Masked(unit) => format!(
                "printf '%s\\n%s\\n' \"$(systemctl is-enabled {unit} 2>&1)\" \"$(systemctl is-active {unit} 2>&1)\""
            ),
            Self::Scrub(_) => format!(
                "systemctl show {SCRUB_TIMER} -p TimersCalendar -p RandomizedDelayUSec -p ActiveState"
            ),
            Self::Cgroup(cgroup) => if_present(BENCHER_CGROUP, &cgroup.read()),
            Self::StateDir => if_present(
                STATE_DIR,
                &format!(
                    "stat -L -c '%a %U:%G %F' {STATE_DIR} && findmnt -n -o OPTIONS -T {STATE_DIR}"
                ),
            ),
            Self::Binary(_) => if_present(RUNNER_BINARY, &format!("sha256sum {RUNNER_BINARY}")),
            // The unit file itself, since `systemctl show` gives the drop-in's command, then every other drop-in's.
            Self::Vendor { unit, .. } => format!(
                "(grep '^ExecStart=' \"$(systemctl show -p FragmentPath --value {unit})\" && for drop_in in $(systemctl show -p DropInPaths --value {unit}); do [ \"$drop_in\" = {ours} ] || grep -EH '^[[:space:]]*ExecStart[[:space:]]*=' \"$drop_in\" || [ $? = 1 ] || exit; done)",
                ours = quiet_drop_in_path(unit),
            ),
        }
    }

    /// Why the row's value is not the desired one, if it is not.
    fn problem(
        &self,
        output: &Output,
        published: &mut dyn FnMut(UpdateChannel) -> anyhow::Result<Sha256>,
    ) -> Option<String> {
        let text = output.text.trim_end();
        match output.status {
            Some(0) => {},
            Some(status) => return Some(format!("could not be read (exit {status}): {text}")),
            None => return Some("could not be read (no exit status)".to_owned()),
        }
        if text == ABSENT {
            return Some(ABSENT.to_owned());
        }
        match self {
            Self::File(file) => file.problem(text),
            Self::Masked(_) => masked_problem(text),
            Self::Scrub(Some(day)) => timer_problem(text, *day),
            Self::Scrub(None) => Some("runners.json sets no scrub_day for this runner".to_owned()),
            Self::Cgroup(Cgroup::Controllers) => controllers_problem(text),
            Self::Cgroup(Cgroup::Cpus) => cpus_problem(text),
            Self::Cgroup(Cgroup::Partition) => {
                text.contains("invalid").then(|| format!("is `{text}`"))
            },
            Self::StateDir => state_dir_problem(text),
            Self::Binary(channel) => binary_problem(text, *channel, published),
            Self::Vendor { command, .. } => vendor_problem(text, command),
        }
    }

    /// The command that sets the row, for the rows `host` sets.
    fn write(&self) -> Option<String> {
        match self {
            Self::File(file) => file.write(),
            // Stopping a running unit leaves it failed, which reads the whole system as degraded.
            Self::Masked(unit) => Some(format!(
                "systemctl mask --now {unit} && if systemctl is-failed --quiet {unit}; then systemctl reset-failed {unit}; fi"
            )),
            Self::Scrub(day) => scrub_write((*day)?),
            Self::Cgroup(_) | Self::StateDir | Self::Binary(_) | Self::Vendor { .. } => None,
        }
    }

    fn reload(&self) -> Option<&'static str> {
        if let Self::File(file) = self {
            file.reload
        } else {
            None
        }
    }
}

impl Cgroup {
    fn read(self) -> String {
        match self {
            Self::Controllers => format!("cat {BENCHER_CGROUP}/cgroup.subtree_control"),
            Self::Cpus => {
                format!("paste -d ' ' {BENCHER_CGROUP}/cpuset.cpus.effective {ISOLATED_CPUS}")
            },
            Self::Partition => format!("cat {BENCHER_CGROUP}/cpuset.cpus.partition"),
        }
    }
}

impl File {
    fn root(path: &str, mode: u32, content: Content, reload: Option<&'static str>) -> Self {
        Self {
            path: path.to_owned(),
            mode,
            user: ROOT.to_owned(),
            group: ROOT.to_owned(),
            content,
            reload,
        }
    }

    fn read(&self) -> String {
        let Self { path, .. } = self;
        let stat = format!("stat -c '%a %U:%G' {path}");
        if_present(
            path,
            &match &self.content {
                Content::Text(_) => format!("{stat} && cat {path}"),
                Content::Secret => stat,
            },
        )
    }

    fn problem(&self, text: &str) -> Option<String> {
        let (stat, content) = text.split_once('\n').unwrap_or((text, ""));
        let mut problems = Vec::new();
        let want = format!("{:o} {}:{}", self.mode, self.user, self.group);
        if stat != want {
            problems.push(format!("is `{stat}`, want `{want}`"));
        }
        if let Content::Text(want) = &self.content
            && content != want.trim_end()
        {
            problems.push("has other content".to_owned());
        }
        (!problems.is_empty()).then(|| problems.join("; "))
    }

    fn write(&self) -> Option<String> {
        let Content::Text(content) = &self.content else {
            return None;
        };
        let Self {
            path,
            mode,
            user,
            group,
            ..
        } = self;
        Some(format!(
            "install -D -m {mode:o} -o {user} -g {group} /dev/stdin {path} << 'ROW_EOF'\n{content}\nROW_EOF"
        ))
    }
}

fn frame(index: usize) -> String {
    format!("{FRAME}{index}")
}

fn if_present(path: &str, command: &str) -> String {
    format!("if [ -e {path} ]; then {command}; else echo {ABSENT}; fi")
}

/// A masked unit that was running reads `failed` once stopped.
fn masked_problem(text: &str) -> Option<String> {
    let (enabled, active) = text.split_once('\n').unwrap_or((text, ""));
    let mut problems = Vec::new();
    if enabled != "masked" {
        problems.push(format!("is {enabled}"));
    }
    if !matches!(active, "inactive" | "failed") {
        problems.push(format!("is {active}"));
    }
    (!problems.is_empty()).then(|| problems.join(" and "))
}

fn controllers_problem(text: &str) -> Option<String> {
    let enabled: BTreeSet<&str> = text.split_whitespace().collect();
    (enabled != BTreeSet::from(RUNNER_CONTROLLERS))
        .then(|| format!("enables `{text}`, want `{}`", RUNNER_CONTROLLERS.join(" ")))
}

/// Why the scrub timer is not active on `day` alone with no random delay, if it is not.
fn timer_problem(show: &str, day: ScrubDay) -> Option<String> {
    let calendars: Vec<&str> = show
        .lines()
        .filter_map(|line| line.strip_prefix("TimersCalendar="))
        .flat_map(|timers| timers.split("OnCalendar=").skip(1))
        .map(|calendar| calendar.split(';').next().unwrap_or_default().trim())
        .collect();
    let property = |name: &str| {
        show.lines()
            .find_map(|line| line.strip_prefix(name)?.strip_prefix('='))
            .map(str::trim)
    };
    let delay = property("RandomizedDelayUSec");
    let state = property("ActiveState");
    let want = on_calendar(day);
    if calendars != [want.as_str()] {
        Some(format!("runs on [{}], want {want}", calendars.join(", ")))
    } else if delay != Some("0") {
        Some(format!(
            "has a random delay of {}",
            delay.unwrap_or("unknown")
        ))
    } else if state != Some("active") {
        Some(format!("is {}", state.unwrap_or("in an unknown state")))
    } else {
        None
    }
}

/// The command that writes the drop-in replacing every calendar of the scrub timer with `day` and starts the timer, with its stamp set to now first, so neither the reload nor the start finds a scrub day it missed; a failed step leaves the timer running only if it was.
fn scrub_write(day: ScrubDay) -> Option<String> {
    let install = File::root(
        SCRUB_DROP_IN,
        0o644,
        Content::Text(format!(
            "[Timer]\nOnCalendar=\nOnCalendar={}\nRandomizedDelaySec=0",
            on_calendar(day)
        )),
        None,
    )
    .write()?;
    Some(format!(
        "set -e
if systemctl is-active --quiet {SCRUB_TIMER}; then active=1; else active=; fi
trap 'if [ -n \"$active\" ]; then systemctl start {SCRUB_TIMER}; fi' EXIT
touch {SCRUB_STAMP}
systemctl stop {SCRUB_TIMER}
{install}
systemctl daemon-reload
trap - EXIT
systemctl start {SCRUB_TIMER}"
    ))
}

/// The calendar in the normal form `systemctl show` prints it.
fn on_calendar(day: ScrubDay) -> String {
    format!("*-*-{:02} 02:00:00", u8::from(day))
}

/// `text` is the effective CPU list, a space, and the isolated CPU list, which is empty without isolation.
fn cpus_problem(text: &str) -> Option<String> {
    let (effective, isolated) = text.split_once(' ').unwrap_or((text, ""));
    let (Some(effective), Some(isolated)) = (cpu_set(effective), cpu_set(isolated)) else {
        return Some(format!("has unreadable CPU lists `{text}`"));
    };
    (effective != isolated).then(|| {
        format!(
            "has CPUs {}, but the kernel isolates {}",
            cpus(&effective),
            cpus(&isolated)
        )
    })
}

fn cpu_set(list: &str) -> Option<BTreeSet<u32>> {
    if list.is_empty() {
        Some(BTreeSet::new())
    } else {
        parse_cpu_list(list)
    }
}

fn cpus(set: &BTreeSet<u32>) -> String {
    if set.is_empty() {
        "none".to_owned()
    } else {
        format_cpu_list(set)
    }
}

fn state_dir_problem(text: &str) -> Option<String> {
    let (stat, options) = text.split_once('\n').unwrap_or((text, ""));
    let mut problems = Vec::new();
    if stat != STATE_DIR_STAT {
        problems.push(format!("is `{stat}`, want `{STATE_DIR_STAT}`"));
    }
    for option in ["nodev", "noexec"] {
        if options.split(',').any(|mounted| mounted == option) {
            problems.push(format!("is mounted {option}"));
        }
    }
    (!problems.is_empty()).then(|| problems.join("; "))
}

fn quiet_drop_in_path(unit: &str) -> String {
    format!("/etc/systemd/system/{unit}.d/{QUIET_DROP_IN}")
}

/// Waits for the runner's job lock, behind a marker of its own (`%n`, the unit) that the runner pauses for.
fn quiet_drop_in(command: &str) -> String {
    let marker = format!("{RUN_DIR}/{MARKER}.%n");
    format!(
        "[Service]
ExecStartPre=+/usr/bin/install -d -m 0700 {RUN_DIR}
ExecStartPre=+/usr/bin/touch {marker}
ExecStart=
ExecStart=/usr/bin/flock -F {RUN_DIR}/{JOB_LOCK} {command}
ExecStopPost=+/usr/bin/rm -f {marker}
TimeoutStartSec=infinity"
    )
}

/// `text` is the unit file's `ExecStart=` lines, which must be the one command the drop-in wraps, then any other
/// drop-in's, after its path, of which there must be none.
fn vendor_problem(text: &str, command: &str) -> Option<String> {
    let (lines, others): (Vec<&str>, Vec<&str>) = text
        .lines()
        .partition(|line| line.starts_with("ExecStart="));
    let want = format!("ExecStart={command}");
    let mut problems = Vec::new();
    if lines != [want.as_str()] {
        problems.push(format!(
            "runs `{}`, but the drop-in wraps `{want}`",
            lines.join("`, `")
        ));
    }
    let mut drop_ins: Vec<&str> = others
        .into_iter()
        .map(|line| line.split_once(':').map_or(line, |(path, _)| path))
        .collect();
    drop_ins.dedup();
    problems.extend(
        drop_ins
            .into_iter()
            .map(|path| format!("`{path}` also sets `ExecStart=`")),
    );
    (!problems.is_empty()).then(|| problems.join("; "))
}

fn binary_problem(
    text: &str,
    channel: UpdateChannel,
    published: &mut dyn FnMut(UpdateChannel) -> anyhow::Result<Sha256>,
) -> Option<String> {
    let installed = text.split_whitespace().next().unwrap_or_default();
    match published(channel) {
        Ok(published) => (!installed.eq_ignore_ascii_case(published.as_ref())).then(|| {
            format!("has sha256 {installed}, but the {channel} release publishes {published}")
        }),
        Err(error) => Some(format!(
            "could not fetch the {channel} release checksum: {error:#}"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_checksum(channel: UpdateChannel) -> anyhow::Result<Sha256> {
        anyhow::bail!("the {channel} checksum was not fetched")
    }

    fn problem(row: &Row, text: &str) -> Option<String> {
        let output = Output {
            text: text.to_owned(),
            status: Some(0),
        };
        row.problem(&output, &mut no_checksum)
    }

    #[test]
    fn a_file_matches_its_mode_owner_and_content() {
        let row = Row::File(File::root(
            "/etc/x",
            0o644,
            Content::Text("a\nb".to_owned()),
            None,
        ));
        assert_eq!(problem(&row, "644 root:root\na\nb\n"), None);
        assert_eq!(
            problem(&row, "600 root:root\na\nb").as_deref(),
            Some("is `600 root:root`, want `644 root:root`")
        );
        assert_eq!(
            problem(&row, "644 root:root\na").as_deref(),
            Some("has other content")
        );
        assert_eq!(problem(&row, ABSENT).as_deref(), Some(ABSENT));
    }

    #[test]
    fn an_unreadable_row_fails() {
        let output = Output {
            text: "cat: cpuset.cpus.partition: No such file or directory\n".to_owned(),
            status: Some(1),
        };
        assert_eq!(
            Row::Cgroup(Cgroup::Partition)
                .problem(&output, &mut no_checksum)
                .as_deref(),
            Some(
                "could not be read (exit 1): cat: cpuset.cpus.partition: No such file or directory"
            )
        );
    }

    #[test]
    fn a_masked_unit_is_also_stopped() {
        let row = Row::Masked("motd-news.timer");
        assert_eq!(problem(&row, "masked\ninactive"), None);
        // A timer that was running reads `failed` once `systemctl mask --now` stops it.
        assert_eq!(problem(&row, "masked\nfailed"), None);
        assert_eq!(
            problem(&row, "enabled\nactive").as_deref(),
            Some("is enabled and is active")
        );
        assert_eq!(
            problem(&row, "masked\nactive").as_deref(),
            Some("is active")
        );
        assert_eq!(
            problem(&row, "not-found\ninactive").as_deref(),
            Some("is not-found")
        );
    }

    #[test]
    fn the_bencher_cgroup_enables_only_the_runner_controllers() {
        let row = Row::Cgroup(Cgroup::Controllers);
        assert_eq!(problem(&row, "cpuset memory pids"), None);
        assert_eq!(
            problem(&row, "cpuset cpu io memory pids").as_deref(),
            Some("enables `cpuset cpu io memory pids`, want `cpuset memory pids`")
        );
        assert_eq!(
            problem(&row, "memory pids").as_deref(),
            Some("enables `memory pids`, want `cpuset memory pids`")
        );
    }

    #[test]
    fn the_bencher_cgroup_runs_on_exactly_the_isolated_cpus() {
        let row = Row::Cgroup(Cgroup::Cpus);
        assert_eq!(problem(&row, "1-5 1-5"), None);
        assert_eq!(problem(&row, "1-5 1,2,3,4,5"), None);
        assert_eq!(
            problem(&row, "1-5 1-4").as_deref(),
            Some("has CPUs 1-5, but the kernel isolates 1-4")
        );
        assert_eq!(
            problem(&row, "0-11 \n").as_deref(),
            Some("has CPUs 0-11, but the kernel isolates none")
        );
    }

    #[test]
    fn only_an_invalid_partition_fails() {
        let row = Row::Cgroup(Cgroup::Partition);
        for level in ["member", "root", "isolated"] {
            assert_eq!(problem(&row, level), None, "{level}");
        }
        assert_eq!(
            problem(&row, "isolated invalid (Cpu list not exclusive)").as_deref(),
            Some("is `isolated invalid (Cpu list not exclusive)`")
        );
    }

    #[test]
    fn the_state_dir_is_private_on_a_mount_that_runs_guests() {
        let row = Row::StateDir;
        assert_eq!(
            problem(
                &row,
                "700 root:root directory\nrw,relatime,errors=remount-ro"
            ),
            None
        );
        assert_eq!(
            problem(&row, "755 root:root directory\nrw,relatime").as_deref(),
            Some("is `755 root:root directory`, want `700 root:root directory`")
        );
        assert_eq!(
            problem(
                &row,
                "700 root:root directory\nrw,nosuid,nodev,noexec,relatime"
            )
            .as_deref(),
            Some("is mounted nodev; is mounted noexec")
        );
    }

    #[test]
    fn the_binary_matches_its_channel_release() {
        const CANARY: &str = "0000000000000000000000000000000000000000000000000000000000000002";
        const STABLE: &str = "0000000000000000000000000000000000000000000000000000000000000001";
        let installed = Output {
            text: format!("{CANARY}  {RUNNER_BINARY}\n"),
            status: Some(0),
        };
        let mut fetched = Vec::new();
        let mut published = |channel| {
            fetched.push(channel);
            Ok(match channel {
                UpdateChannel::Canary => CANARY,
                UpdateChannel::Stable => STABLE,
            }
            .parse()?)
        };
        assert_eq!(
            Row::Binary(UpdateChannel::Canary).problem(&installed, &mut published),
            None
        );
        assert_eq!(
            Row::Binary(UpdateChannel::Stable).problem(&installed, &mut published),
            Some(format!(
                "has sha256 {CANARY}, but the stable release publishes {STABLE}"
            ))
        );
        assert_eq!(
            Row::Binary(UpdateChannel::Canary)
                .problem(
                    &Output {
                        text: format!("{ABSENT}\n"),
                        status: Some(0),
                    },
                    &mut published
                )
                .as_deref(),
            Some(ABSENT)
        );
        assert_eq!(fetched, [UpdateChannel::Canary, UpdateChannel::Stable]);
        assert_eq!(
            Row::Binary(UpdateChannel::Canary)
                .problem(&installed, &mut no_checksum)
                .as_deref(),
            Some(
                "could not fetch the canary release checksum: the canary checksum was not fetched"
            )
        );
    }

    fn day(day: u8) -> ScrubDay {
        ScrubDay::try_from(day).unwrap()
    }

    #[test]
    fn the_scrub_timer_runs_only_on_the_scrub_day() {
        // `systemctl show` output from systemd 255.
        let row = Row::Scrub(Some(day(5)));
        assert_eq!(
            problem(
                &row,
                "TimersCalendar={ OnCalendar=*-*-05 02:00:00 ; next_elapse=Thu 2026-11-05 02:00:00 UTC }\nRandomizedDelayUSec=0\nActiveState=active"
            ),
            None
        );
        assert_eq!(
            problem(
                &row,
                "TimersCalendar={ OnCalendar=*-*-05 02:00:00 ; next_elapse=(null) }\nRandomizedDelayUSec=0\nActiveState=inactive"
            )
            .as_deref(),
            Some("is inactive")
        );
        assert_eq!(
            problem(
                &row,
                "TimersCalendar={ OnCalendar=Sun *-*-01..07 01:00:00 ; next_elapse=(null) }\nRandomizedDelayUSec=1d"
            )
            .as_deref(),
            Some("runs on [Sun *-*-01..07 01:00:00], want *-*-05 02:00:00")
        );
        assert_eq!(
            problem(
                &row,
                "TimersCalendar={ OnCalendar=*-*-05 02:00:00 ; next_elapse=(null) }\nTimersCalendar={ OnCalendar=Sun *-*-01..07 01:00:00 ; next_elapse=(null) }\nRandomizedDelayUSec=0"
            )
            .as_deref(),
            Some("runs on [*-*-05 02:00:00, Sun *-*-01..07 01:00:00], want *-*-05 02:00:00")
        );
        assert_eq!(
            problem(
                &row,
                "TimersCalendar={ OnCalendar=*-*-05 02:00:00 ; next_elapse=(null) }\nRandomizedDelayUSec=1d"
            )
            .as_deref(),
            Some("has a random delay of 1d")
        );
        assert_eq!(
            problem(&row, "RandomizedDelayUSec=0").as_deref(),
            Some("runs on [], want *-*-05 02:00:00")
        );
        assert_eq!(
            problem(&Row::Scrub(None), "RandomizedDelayUSec=0").as_deref(),
            Some("runners.json sets no scrub_day for this runner")
        );
    }

    #[test]
    fn host_pins_the_scrub_day_before_a_mask_reloads_systemd() {
        let desired = Desired::new(UpdateChannel::Stable, Some(day(5)));
        let (writes, _) = desired.writes(&desired.check(&BTreeMap::new(), &mut no_checksum));
        let scrub = writes
            .iter()
            .position(|write| write.contains(SCRUB_DROP_IN))
            .unwrap();
        let first_mask = writes
            .iter()
            .position(|write| write.starts_with("systemctl mask"))
            .unwrap();
        assert!(scrub < first_mask, "{writes:#?}");
    }

    #[test]
    fn a_vendor_command_that_changed_fails_the_audit() {
        // Kills a drift check that passes a changed command, a prefix of it, or
        // a second command, any of which the drop-in would not run.
        let row = Row::Vendor {
            unit: "fstrim.service",
            command: "/sbin/fstrim --verbose",
        };
        assert_eq!(problem(&row, "ExecStart=/sbin/fstrim --verbose\n"), None);
        assert_eq!(
            problem(&row, "ExecStart=/usr/sbin/fstrim --verbose").as_deref(),
            Some(
                "runs `ExecStart=/usr/sbin/fstrim --verbose`, but the drop-in wraps `ExecStart=/sbin/fstrim --verbose`"
            )
        );
        assert!(problem(&row, "ExecStart=/sbin/fstrim --verbose --all").is_some());
        assert!(problem(&row, "ExecStart=/sbin/fstrim").is_some());
        assert!(
            problem(
                &row,
                "ExecStart=/sbin/fstrim --verbose\nExecStart=/sbin/fstrim --verbose"
            )
            .is_some()
        );
    }

    #[test]
    fn host_writes_each_quiet_drop_in_and_reloads_systemd_once() {
        // Kills a drop-in that takes effect only at the next reboot, a missing
        // unit, and a drift check that `host` would try to write.
        let quiet = Desired(
            Desired::new(UpdateChannel::Stable, Some(day(5)))
                .0
                .into_iter()
                .filter(|row| match row {
                    Row::File(file) => file.path.ends_with(QUIET_DROP_IN),
                    Row::Vendor { .. } => true,
                    Row::Masked(_)
                    | Row::Scrub(_)
                    | Row::Cgroup(_)
                    | Row::StateDir
                    | Row::Binary(_) => false,
                })
                .collect(),
        );
        let (writes, reloads) = quiet.writes(&quiet.check(&BTreeMap::new(), &mut no_checksum));
        assert_eq!(writes.len(), QUIET_UNITS.len(), "{writes:#?}");
        let written: Vec<&str> = writes
            .iter()
            .filter_map(|write| {
                write
                    .split_whitespace()
                    .find(|word| word.ends_with(QUIET_DROP_IN))
            })
            .collect();
        assert_eq!(
            written,
            [
                "/etc/systemd/system/apt-daily.service.d/zz-bencher-quiet.conf",
                "/etc/systemd/system/apt-daily-upgrade.service.d/zz-bencher-quiet.conf",
                "/etc/systemd/system/fstrim.service.d/zz-bencher-quiet.conf",
                "/etc/systemd/system/mdcheck_start.service.d/zz-bencher-quiet.conf",
                "/etc/systemd/system/mdcheck_continue.service.d/zz-bencher-quiet.conf",
            ]
        );
        assert_eq!(reloads, ["systemctl daemon-reload"]);
    }

    #[test]
    fn each_drop_in_removes_the_marker_it_touches_where_the_runner_looks() {
        // Kills a drop-in that removes another marker than it touched, which
        // pauses the runner until a reboot, and a marker the runner never
        // sees, which lets maintenance wait unseen.
        for (unit, command) in QUIET_UNITS {
            let drop_in = quiet_drop_in(command);
            let path_after = |program: &str| {
                drop_in
                    .lines()
                    .find(|line| line.contains(program))
                    .and_then(|line| line.split_whitespace().last())
                    .unwrap_or_else(|| panic!("no {program} in {drop_in}"))
                    .replace("%n", unit)
            };
            let touched = path_after("/usr/bin/touch ");
            assert_eq!(path_after("/usr/bin/rm "), touched, "{unit}");

            let marker = std::path::Path::new(&touched);
            assert_eq!(
                marker.parent(),
                Some(std::path::Path::new(RUN_DIR)),
                "{unit}"
            );
            let run_dir = tempfile::tempdir().unwrap();
            std::fs::write(run_dir.path().join(marker.file_name().unwrap()), b"").unwrap();
            assert!(
                bencher_runner::maintenance::marker_present(run_dir.path()),
                "{touched}"
            );
        }
    }

    #[cfg(target_os = "linux")]
    mod linux {
        use std::fs::{self, Permissions};
        use std::os::unix::fs::PermissionsExt as _;
        use std::process::Command;

        use camino::Utf8Path;

        use super::super::*;
        use super::no_checksum;
        use crate::task::ssh::write_executable;

        /// The user and group running the test, which its files belong to.
        fn owner() -> (String, String) {
            let id = |flag| {
                let output = Command::new("id").arg(flag).output().unwrap();
                String::from_utf8(output.stdout).unwrap().trim().to_owned()
            };
            (id("-un"), id("-gn"))
        }

        fn sh(dir: &Utf8Path, script: &str) -> anyhow::Result<String> {
            let output = Command::new("sh")
                .arg("-c")
                .arg(script)
                .current_dir(dir)
                .output()?;
            anyhow::ensure!(output.status.success(), "{output:?}");
            Ok(String::from_utf8(output.stdout)?)
        }

        fn write(path: &Utf8Path, content: &str, mode: u32) {
            fs::write(path, content).unwrap();
            fs::set_permissions(path, Permissions::from_mode(mode)).unwrap();
        }

        fn mode(path: &Utf8Path) -> u32 {
            fs::metadata(path).unwrap().permissions().mode() & 0o7777
        }

        #[test]
        fn host_writes_what_differs_then_reloads_once() {
            let dir = tempfile::tempdir().unwrap();
            let dir = Utf8Path::from_path(dir.path()).unwrap();
            let (user, group) = owner();
            let file = |name: &str, mode, content: &str| {
                Row::File(File {
                    path: dir.join(name).to_string(),
                    mode,
                    user: user.clone(),
                    group: group.clone(),
                    content: Content::Text(content.to_owned()),
                    // Records what the reload sees, so it must follow every write.
                    reload: Some("cat changed >> reloads"),
                })
            };
            write(&dir.join("same"), "same\n", 0o644);
            write(&dir.join("changed"), "old\n", 0o644);
            write(&dir.join("private"), "private\n", 0o644);
            let desired = Desired(vec![
                file("same", 0o644, "same"),
                file("new/absent", 0o640, "absent"),
                file("changed", 0o644, "changed"),
                file("private", 0o600, "private"),
            ]);

            let mut runs = Vec::new();
            let Applied { written, findings } = desired
                .apply(
                    |script| sh(dir, script),
                    |command| {
                        runs.push(command.to_owned());
                        sh(dir, command)
                    },
                    &mut no_checksum,
                )
                .unwrap();
            assert!(
                findings.iter().all(|finding| !finding.failed()),
                "{findings:?}"
            );
            assert_eq!(written, 3);
            assert_eq!(runs.len(), 4, "3 writes and 1 reload: {runs:?}");
            assert_eq!(
                fs::read_to_string(dir.join("reloads")).unwrap(),
                "changed\n"
            );
            assert_eq!(
                fs::read_to_string(dir.join("new/absent")).unwrap(),
                "absent\n"
            );
            assert_eq!(mode(&dir.join("new/absent")), 0o640);
            assert_eq!(mode(&dir.join("private")), 0o600);

            let mut runs = Vec::new();
            let applied = desired
                .apply(
                    |script| sh(dir, script),
                    |command| {
                        runs.push(command.to_owned());
                        sh(dir, command)
                    },
                    &mut no_checksum,
                )
                .unwrap();
            assert_eq!(applied.written, 0);
            assert_eq!(runs, Vec::<String>::new());
        }

        #[test]
        fn host_never_reads_nor_writes_the_runner_key() {
            let dir = tempfile::tempdir().unwrap();
            let dir = Utf8Path::from_path(dir.path()).unwrap();
            let key = dir.join("key.env");
            write(&key, "BENCHER_RUNNER_KEY=not-a-real-key\n", 0o644);

            let (user, group) = owner();
            let table = Desired::new(UpdateChannel::Canary, None);
            let read = sh(dir, &table.script().replace(KEY_FILE, key.as_str())).unwrap();
            assert!(!read.contains("not-a-real-key"), "{read}");
            let findings = table.check(&framed::parse(&read), &mut no_checksum);
            let key_row = findings
                .iter()
                .find(|finding| finding.name == KEY_FILE)
                .unwrap();
            assert_eq!(
                key_row.problem,
                Some(format!("is `644 {user}:{group}`, want `600 root:root`"))
            );

            let desired = Desired(vec![Row::File(File {
                path: key.to_string(),
                mode: 0o600,
                user: user.clone(),
                group: group.clone(),
                content: Content::Secret,
                reload: None,
            })]);
            let mut runs = Vec::new();
            let Applied { written, findings } = desired
                .apply(
                    |script| sh(dir, script),
                    |command| {
                        runs.push(command.to_owned());
                        sh(dir, command)
                    },
                    &mut no_checksum,
                )
                .unwrap();
            assert_eq!(written, 0);
            assert_eq!(runs, Vec::<String>::new());
            assert_eq!(
                findings,
                [Finding {
                    name: key.to_string(),
                    set_by: SetBy::Start,
                    problem: Some(format!(
                        "is `644 {user}:{group}`, want `600 {user}:{group}`"
                    )),
                }]
            );
            assert!(!findings.iter().any(Finding::host_unset));
            assert_eq!(
                fs::read_to_string(&key).unwrap(),
                "BENCHER_RUNNER_KEY=not-a-real-key\n"
            );
        }

        #[test]
        fn host_pins_the_scrub_day_with_its_stamp_touched_before_the_timer_stops() {
            let dir = tempfile::tempdir().unwrap();
            let dir = Utf8Path::from_path(dir.path()).unwrap();
            let write_scrub = scrub_stand_ins(dir);
            let pinned = "\
systemctl is-active --quiet mdcheck_start.timer
touch /var/lib/systemd/timers/stamp-mdcheck_start.timer
systemctl stop mdcheck_start.timer
install -D -m 644 -o root -g root /dev/stdin /etc/systemd/system/mdcheck_start.timer.d/zz-bencher-scrub.conf
[Timer]
OnCalendar=
OnCalendar=*-*-05 02:00:00
RandomizedDelaySec=0
systemctl daemon-reload
systemctl start mdcheck_start.timer
";

            assert_eq!(write_scrub().unwrap(), pinned);
            write(&dir.join("active"), "", 0o644);
            assert_eq!(write_scrub().unwrap(), pinned);
        }

        #[test]
        fn a_failed_scrub_write_leaves_the_timer_as_it_found_it() {
            let dir = tempfile::tempdir().unwrap();
            let dir = Utf8Path::from_path(dir.path()).unwrap();
            let write_scrub = scrub_stand_ins(dir);
            let touched = "\
systemctl is-active --quiet mdcheck_start.timer
touch /var/lib/systemd/timers/stamp-mdcheck_start.timer
";
            let installed = format!(
                "{touched}systemctl stop mdcheck_start.timer
install -D -m 644 -o root -g root /dev/stdin /etc/systemd/system/mdcheck_start.timer.d/zz-bencher-scrub.conf
"
            );
            let start = "systemctl start mdcheck_start.timer\n";

            write(&dir.join("fail-touch"), "", 0o644);
            assert_eq!(write_scrub().unwrap_err(), touched);
            write(&dir.join("active"), "", 0o644);
            assert_eq!(write_scrub().unwrap_err(), format!("{touched}{start}"));

            fs::remove_file(dir.join("fail-touch")).unwrap();
            write(&dir.join("fail-install"), "", 0o644);
            assert_eq!(write_scrub().unwrap_err(), format!("{installed}{start}"));
            fs::remove_file(dir.join("active")).unwrap();
            assert_eq!(write_scrub().unwrap_err(), installed);
        }

        /// Recording stand-ins for the scrub write's programs, and a run of the write that returns their calls, as `Err` when it fails; the timer is active while `active` exists, and `fail-<program>` fails that program.
        fn scrub_stand_ins(dir: &Utf8Path) -> impl Fn() -> Result<String, String> {
            for program in ["systemctl", "install", "touch"] {
                write_executable(
                    &dir.join(program),
                    &format!(
                        r#"#!/bin/sh
dir="$(dirname "$0")"
echo "{program} $*" >> "$dir/calls"
[ -e "$dir/fail-{program}" ] && exit 1
case "{program} $1" in
  "install -D") cat >> "$dir/calls" ;;
  "systemctl is-active") [ -e "$dir/active" ] ;;
esac
"#
                    ),
                )
                .unwrap();
            }
            let desired = Desired(vec![Row::Scrub(Some(super::day(5))), Row::Scrub(None)]);
            let findings: Vec<Finding> = desired
                .0
                .iter()
                .map(|row| Finding {
                    name: row.name(),
                    set_by: row.set_by(),
                    problem: Some("differs".to_owned()),
                })
                .collect();
            let (writes, reloads) = desired.writes(&findings);
            assert_eq!(writes.len(), 1);
            assert_eq!(reloads, Vec::<&str>::new());
            let dir = dir.to_owned();
            move || {
                let wrote = sh(&dir, &format!("PATH={dir}:$PATH\n{}", writes[0]));
                let calls = fs::read_to_string(dir.join("calls")).unwrap();
                fs::remove_file(dir.join("calls")).unwrap();
                if wrote.is_ok() { Ok(calls) } else { Err(calls) }
            }
        }

        #[test]
        fn host_masks_and_stops_every_listed_unit_leaving_none_failed() {
            let dir = tempfile::tempdir().unwrap();
            let dir = Utf8Path::from_path(dir.path()).unwrap();
            // A stand-in for systemctl that keeps the masked and failed units in files: every timer runs, so stopping it leaves it failed, and `gpu-manager` is not installed, so it cannot be reset.
            write_executable(
                &dir.join("systemctl"),
                r#"#!/bin/sh
state="$(dirname "$0")/masked"
failed="$(dirname "$0")/failed"
masked() { grep -qx "$1" "$state" 2>/dev/null; }
failed() { grep -qx "$1" "$failed" 2>/dev/null; }
case "$1 $2" in
  "is-enabled $2") if masked "$2"; then echo masked; exit 1; else echo enabled; fi ;;
  "is-active $2") if failed "$2"; then echo failed; exit 3; elif masked "$2"; then echo inactive; exit 3; else echo active; fi ;;
  "mask --now") echo "$3" >> "$state"; case "$3" in *.timer) echo "$3" >> "$failed" ;; esac ;;
  "is-failed --quiet") failed "$3" ;;
  "reset-failed gpu-manager.service") exit 1 ;;
  "reset-failed $2") grep -vx "$2" "$failed" > "$failed.new"; mv "$failed.new" "$failed" ;;
  *) exit 1 ;;
esac
"#,
            )
            .unwrap();
            let fake = |script: &str| sh(dir, &format!("PATH={dir}:$PATH\n{script}"));
            let masks = || {
                Desired(
                    Desired::new(UpdateChannel::Canary, None)
                        .0
                        .into_iter()
                        .filter(|row| matches!(row, Row::Masked(_)))
                        .collect(),
                )
            };
            let masked = || fs::read_to_string(dir.join("masked")).unwrap();
            let failed = || fs::read_to_string(dir.join("failed")).unwrap();

            let findings = masks()
                .apply(fake, fake, &mut no_checksum)
                .unwrap()
                .findings;
            assert_eq!(findings.len(), MASKED_UNITS.len());
            assert!(
                findings.iter().all(|finding| !finding.failed()),
                "{findings:?}"
            );
            assert_eq!(masked(), format!("{}\n", MASKED_UNITS.join("\n")));
            assert_eq!(failed(), "");

            masks().apply(fake, fake, &mut no_checksum).unwrap();
            assert_eq!(masked(), format!("{}\n", MASKED_UNITS.join("\n")));
            assert_eq!(failed(), "");
        }

        #[test]
        fn the_cpus_read_pairs_effective_with_isolated_cpus() {
            let dir = tempfile::tempdir().unwrap();
            let dir = Utf8Path::from_path(dir.path()).unwrap();
            fs::create_dir(dir.join("bencher")).unwrap();
            fs::write(dir.join("bencher/cpuset.cpus.effective"), "1-5\n").unwrap();
            let desired = Desired(vec![Row::Cgroup(Cgroup::Cpus)]);
            let script = desired
                .script()
                .replace(BENCHER_CGROUP, dir.join("bencher").as_str())
                .replace(ISOLATED_CPUS, dir.join("isolated").as_str());
            let check =
                || desired.check(&framed::parse(&sh(dir, &script).unwrap()), &mut no_checksum);

            fs::write(dir.join("isolated"), "\n").unwrap();
            assert_eq!(
                check()
                    .into_iter()
                    .map(|finding| finding.problem)
                    .collect::<Vec<_>>(),
                [Some(
                    "has CPUs 1-5, but the kernel isolates none".to_owned()
                )]
            );
            fs::write(dir.join("isolated"), "1-5\n").unwrap();
            assert!(!check().iter().any(Finding::failed));
        }

        #[test]
        fn the_vendor_command_is_read_from_the_unit_file() {
            // Kills a read of the effective command, which is the drop-in's
            // own once it is installed, so a vendor change never shows.
            let dir = tempfile::tempdir().unwrap();
            let dir = Utf8Path::from_path(dir.path()).unwrap();
            let unit = dir.join("fstrim.service");
            write_executable(
                &dir.join("systemctl"),
                &format!(
                    r#"#!/bin/sh
case "$*" in
  "show -p FragmentPath --value fstrim.service") echo {unit} ;;
  "show -p DropInPaths --value fstrim.service") echo ;;
  "show -p ExecStart --value fstrim.service") echo "/usr/bin/flock -F /run/bencher/job.lock /sbin/fstrim" ;;
  *) exit 1 ;;
esac
"#
                ),
            )
            .unwrap();
            let desired = Desired(vec![Row::Vendor {
                unit: "fstrim.service",
                command: "/sbin/fstrim",
            }]);
            let script = format!("PATH={dir}:$PATH\n{}", desired.script());
            let check =
                || desired.check(&framed::parse(&sh(dir, &script).unwrap()), &mut no_checksum);

            write(
                &unit,
                "[Service]\nType=oneshot\nExecStartPre=-/usr/bin/true\nExecStart=/sbin/fstrim\n",
                0o644,
            );
            assert!(!check().iter().any(Finding::failed), "{:?}", check());
            write(&unit, "[Service]\nExecStart=/usr/sbin/fstrim\n", 0o644);
            assert!(check().iter().all(Finding::failed), "{:?}", check());
        }

        #[test]
        fn another_drop_in_that_sets_the_command_fails_the_audit() {
            // Kills an audit that misses another drop-in's `ExecStart=`, which
            // either the drop-in silently replaces or replaces the drop-in, one
            // that fails on the drop-in itself or on a drop-in that sets no
            // command, and one that passes a drop-in it could not read.
            let dir = tempfile::tempdir().unwrap();
            let dir = Utf8Path::from_path(dir.path()).unwrap();
            let unit = dir.join("fstrim.service");
            let drop_ins = dir.join("drop-ins");
            write(&unit, "[Service]\nExecStart=/sbin/fstrim\n", 0o644);
            write_executable(
                &dir.join("systemctl"),
                &format!(
                    r#"#!/bin/sh
case "$*" in
  "show -p FragmentPath --value fstrim.service") echo {unit} ;;
  "show -p DropInPaths --value fstrim.service") cat {drop_ins} ;;
  *) exit 1 ;;
esac
"#
                ),
            )
            .unwrap();
            let desired = Desired(vec![Row::Vendor {
                unit: "fstrim.service",
                command: "/sbin/fstrim",
            }]);
            let script = format!("PATH={dir}:$PATH\n{}", desired.script());
            let check = |paths: &[&Utf8Path]| {
                let paths: Vec<&str> = paths.iter().map(|path| path.as_str()).collect();
                write(&drop_ins, &paths.join(" "), 0o644);
                let mut findings =
                    desired.check(&framed::parse(&sh(dir, &script).unwrap()), &mut no_checksum);
                findings.pop().unwrap()
            };
            let ours = quiet_drop_in_path("fstrim.service");
            let ours = Utf8Path::new(&ours);
            let quiet = dir.join("nice.conf");
            write(&quiet, "[Service]\nNice=10\n", 0o644);
            let quiet = quiet.as_path();

            assert!(!check(&[ours]).failed(), "{}", check(&[ours]));
            assert!(!check(&[quiet, ours]).failed(), "{}", check(&[quiet, ours]));
            let unread = dir.join("gone.conf");
            let finding = check(&[unread.as_path(), ours]);
            assert!(
                finding
                    .problem
                    .as_deref()
                    .is_some_and(|problem| problem.starts_with("could not be read")),
                "{finding}"
            );
            for content in [
                "[Service]\nExecStart=\nExecStart=/sbin/fstrim --all\n",
                "[Service]\n  ExecStart = /sbin/fstrim --all\n",
            ] {
                let other = dir.join("override.conf");
                write(&other, content, 0o644);
                let finding = check(&[other.as_path(), ours]);
                assert_eq!(
                    finding.problem.as_deref(),
                    Some(format!("`{other}` also sets `ExecStart=`").as_str()),
                    "{content:?}"
                );
            }
        }
    }
}
