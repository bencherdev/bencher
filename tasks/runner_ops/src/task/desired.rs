use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use bencher_json::{Sha256, UpdateChannel};

use super::framed::{self, Output};
use super::isolate::{format_cpu_list, parse_cpu_list};
use super::start::KEY_FILE;

pub const SSH_HARDENING_PATH: &str = "/etc/ssh/sshd_config.d/hardening.conf";
pub const UNATTENDED_UPGRADES_PATH: &str = "/etc/apt/apt.conf.d/50unattended-upgrades-local";
pub const AUTO_UPGRADES_PATH: &str = "/etc/apt/apt.conf.d/20auto-upgrades";
pub const KERNEL_PIN_PATH: &str = "/etc/apt/preferences.d/bencher-kernels.pref";
pub const BENCHER_CGROUP: &str = "/sys/fs/cgroup/bencher";
const ISOLATED_CPUS: &str = "/sys/devices/system/cpu/isolated";
/// The controllers the runner enables for its `bencher/` cgroup.
pub const RUNNER_CONTROLLERS: [&str; 3] = ["cpuset", "memory", "pids"];
const STATE_DIR: &str = "/var/lib/bencher-runner";
const STATE_DIR_STAT: &str = "700 root:root directory";
const RUNNER_BINARY: &str = "/usr/local/bin/runner";
const ROOT: &str = "root";
const MASKED_UNITS: [&str; 0] = [];
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
    pub fn new(update_channel: UpdateChannel) -> Self {
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
        ];
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
}

impl fmt::Display for SetBy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Host => "`cargo ops host`",
            Self::Start => "`cargo ops start`",
            Self::Deploy => "`cargo ops deploy`",
            Self::Runner => "the runner",
        })
    }
}

#[derive(Debug)]
enum Row {
    File(File),
    /// A unit masked and stopped, so it never runs beside a Job.
    Masked(&'static str),
    /// The runner's own cgroup, which it sets up at startup.
    Cgroup(Cgroup),
    StateDir,
    /// The installed runner binary, checked against the checksum its channel's release publishes.
    Binary(UpdateChannel),
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
            Self::Cgroup(Cgroup::Controllers) => format!("{BENCHER_CGROUP} controllers"),
            Self::Cgroup(Cgroup::Cpus) => format!("{BENCHER_CGROUP} CPUs"),
            Self::Cgroup(Cgroup::Partition) => format!("{BENCHER_CGROUP} partition"),
            Self::StateDir => STATE_DIR.to_owned(),
            Self::Binary(channel) => format!("{RUNNER_BINARY} ({channel})"),
        }
    }

    fn set_by(&self) -> SetBy {
        match self {
            Self::File(File {
                content: Content::Secret,
                ..
            }) => SetBy::Start,
            Self::File(_) | Self::Masked(_) => SetBy::Host,
            Self::Cgroup(_) | Self::StateDir => SetBy::Runner,
            Self::Binary(_) => SetBy::Deploy,
        }
    }

    /// A read-only command that prints the row's current value.
    fn read(&self) -> String {
        match self {
            Self::File(file) => file.read(),
            Self::Masked(unit) => format!(
                "printf '%s\\n%s\\n' \"$(systemctl is-enabled {unit} 2>&1)\" \"$(systemctl is-active {unit} 2>&1)\""
            ),
            Self::Cgroup(cgroup) => if_present(BENCHER_CGROUP, &cgroup.read()),
            Self::StateDir => if_present(
                STATE_DIR,
                &format!(
                    "stat -L -c '%a %U:%G %F' {STATE_DIR} && findmnt -n -o OPTIONS -T {STATE_DIR}"
                ),
            ),
            Self::Binary(_) => if_present(RUNNER_BINARY, &format!("sha256sum {RUNNER_BINARY}")),
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
            Self::Cgroup(Cgroup::Controllers) => controllers_problem(text),
            Self::Cgroup(Cgroup::Cpus) => cpus_problem(text),
            Self::Cgroup(Cgroup::Partition) => {
                text.contains("invalid").then(|| format!("is `{text}`"))
            },
            Self::StateDir => state_dir_problem(text),
            Self::Binary(channel) => binary_problem(text, *channel, published),
        }
    }

    /// The command that sets the row, for the rows `host` sets.
    fn write(&self) -> Option<String> {
        match self {
            Self::File(file) => file.write(),
            Self::Masked(unit) => Some(format!("systemctl mask --now {unit}")),
            Self::Cgroup(_) | Self::StateDir | Self::Binary(_) => None,
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

    #[cfg(target_os = "linux")]
    mod linux {
        use std::fs::{self, Permissions};
        use std::os::unix::fs::PermissionsExt as _;
        use std::process::Command;

        use camino::Utf8Path;

        use super::super::*;
        use super::no_checksum;

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
            let table = Desired::new(UpdateChannel::Canary);
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
    }
}
