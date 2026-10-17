mod health;
mod snapshot;

use std::collections::BTreeMap;

use bencher_json::RunnerResourceId;

use super::desired::{self, Desired};
use super::download::published_checksum;
use super::framed::{self, Output};
use super::merge_ssh;
use super::ssh::Ssh;
use crate::parser::TaskAudit;
use crate::parser::image_cache_gib::{ImageCacheGib, image_cache_gib_problems};
use crate::parser::refresh_minute::{RefreshMinute, refresh_minute_problems};
use crate::parser::scrub_day::{ScrubDay, scrub_day_problems};
use crate::parser::server::{load_server, load_servers};
use snapshot::{Sections, Snapshot};

#[derive(Debug)]
pub struct Audit {
    runner: Target,
    against: Option<Target>,
}

#[derive(Debug)]
struct Target {
    label: String,
    ssh: Ssh,
    desired: Desired,
}

impl TryFrom<TaskAudit> for Audit {
    type Error = anyhow::Error;

    fn try_from(task: TaskAudit) -> anyhow::Result<Self> {
        let TaskAudit {
            runner,
            server,
            ssh,
            user,
            against,
        } = task;
        let file = runner.as_ref().map(load_server).transpose()?.flatten();
        let update_channel = file
            .as_ref()
            .and_then(|f| f.update_channel)
            .unwrap_or_default();
        let scrub_day = file.as_ref().and_then(|f| f.scrub_day);
        let refresh_minute = file.as_ref().and_then(|f| f.refresh_minute);
        let image_cache_gib = file.as_ref().and_then(|f| f.image_cache_gib);
        let (server, ssh, user) = merge_ssh(file.as_ref(), server, ssh, user)?;
        let label = runner.map_or_else(|| server.clone(), |runner| runner.to_string());
        Ok(Self {
            runner: Target {
                label,
                ssh: Ssh::new(server, ssh, user),
                desired: Desired::new(update_channel, scrub_day, refresh_minute, image_cache_gib),
            },
            against: against.as_ref().map(Target::from_file).transpose()?,
        })
    }
}

impl Audit {
    pub fn exec(self) -> anyhow::Result<()> {
        let mut fleet = Fleet::default();
        for (runner, server) in load_servers()? {
            fleet.scrub_days.push((runner.clone(), server.scrub_day));
            fleet
                .refresh_minutes
                .push((runner.clone(), server.refresh_minute));
            fleet
                .image_cache_gibs
                .push((runner, server.image_cache_gib));
        }
        self.exec_with_fleet(&fleet)
    }

    /// The audit, with `fleet` the runners.json fields that every runner must set.
    fn exec_with_fleet(self, fleet: &Fleet) -> anyhow::Result<()> {
        let Self { runner, against } = self;
        let (sections, frames) = runner.collect()?;
        let snapshot = Snapshot::new(&sections);
        report_size(&runner.label, &snapshot);

        let mut differing = 0;
        let reference = if let Some(reference) = against {
            let (reference_sections, reference_frames) = reference.collect()?;
            let reference_snapshot = Snapshot::new(&reference_sections);
            report_size(&reference.label, &reference_snapshot);
            let diff = snapshot.diff(&reference_snapshot);
            println!("Differences (- {}, + {}):", reference.label, runner.label);
            if diff.is_empty() {
                println!("none");
            }
            for section in &diff {
                print!("{section}");
            }
            differing = diff.len();
            Some((reference, reference_sections, reference_frames))
        } else {
            print!("{snapshot}");
            None
        };

        let mut failed = report_health(&runner.label, &sections);
        let mut unmet = runner.report_desired(&frames);
        if let Some((reference, reference_sections, reference_frames)) = &reference {
            failed += report_health(&reference.label, reference_sections);
            unmet += reference.report_desired(reference_frames);
        }
        failed += report_fleet(
            "Scrub days in runners.json:",
            "every runner scrubs on its own day, at least 2 days from any other",
            &scrub_day_problems(&fleet.scrub_days),
        );
        failed += report_fleet(
            "Refresh minutes in runners.json:",
            "every runner refreshes at its own minute, at least 5 minutes from any other",
            &refresh_minute_problems(&fleet.refresh_minutes),
        );
        failed += report_fleet(
            "Image cache sizes in runners.json:",
            "every runner sets its image_cache_gib",
            &image_cache_gib_problems(&fleet.image_cache_gibs),
        );

        if failed > 0 || unmet > 0 || differing > 0 {
            anyhow::bail!(
                "Audit found {failed} failed health check(s), {unmet} desired state difference(s), and {differing} differing section(s)"
            );
        }
        println!("Audit clean");
        Ok(())
    }
}

/// Each runner's scrub day and refresh minute in runners.json, which no two runners may share, and its image cache size.
#[derive(Debug, Default)]
struct Fleet {
    scrub_days: Vec<(RunnerResourceId, Option<ScrubDay>)>,
    refresh_minutes: Vec<(RunnerResourceId, Option<RefreshMinute>)>,
    image_cache_gibs: Vec<(RunnerResourceId, Option<ImageCacheGib>)>,
}

impl Target {
    /// A reference runner resolves from runners.json alone.
    fn from_file(runner: &RunnerResourceId) -> anyhow::Result<Self> {
        let file = load_server(runner)?
            .ok_or_else(|| anyhow::anyhow!("Runner {runner} is not in runners.json"))?;
        let (server, ssh, user) = merge_ssh(Some(&file), None, None, None)?;
        Ok(Self {
            label: runner.to_string(),
            ssh: Ssh::new(server, ssh, user),
            desired: Desired::new(
                file.update_channel.unwrap_or_default(),
                file.scrub_day,
                file.refresh_minute,
                file.image_cache_gib,
            ),
        })
    }

    /// The snapshot sections and the desired state rows, read in one script.
    fn collect(&self) -> anyhow::Result<(Sections, BTreeMap<String, Output>)> {
        println!("Collecting an audit snapshot from {}...", self.label);
        let script = framed::script(snapshot::frames().chain(self.desired.frames()));
        let mut frames = framed::parse(&self.ssh.run_quiet(&script)?);
        Ok((Sections::take(&mut frames), frames))
    }

    /// Print this runner's desired state and count the rows that differ.
    fn report_desired(&self, frames: &BTreeMap<String, Output>) -> usize {
        desired::report(
            &self.label,
            &self.desired.check(frames, &mut published_checksum),
        )
    }
}

/// Print the health findings for one runner and count the failures.
fn report_health(label: &str, sections: &Sections) -> usize {
    println!("Health of {label}:");
    let findings = health::check(sections);
    for finding in &findings {
        println!("  {finding}");
    }
    findings.iter().filter(|finding| finding.failed()).count()
}

/// Print one runners.json check under `title`, `ok` when it has no problems, and count the failure.
fn report_fleet(title: &str, ok: &str, problems: &[String]) -> usize {
    println!("{title}");
    if problems.is_empty() {
        println!("  ok    {ok}");
    }
    for problem in problems {
        println!("  FAIL  {problem}");
    }
    usize::from(!problems.is_empty())
}

fn report_size(label: &str, snapshot: &Snapshot) {
    let (sections, lines) = snapshot.size();
    println!("Audited {sections} sections ({lines} lines) from {label}");
}

#[cfg(test)]
#[cfg(target_os = "linux")]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;

    use bencher_json::UpdateChannel;
    use camino::Utf8Path;

    use super::snapshot::fake_output;
    use super::*;
    use crate::parser::image_cache_gib::gib;

    #[test]
    fn the_audit_reads_every_desired_row_in_its_one_script() {
        let dir = tempfile::tempdir().unwrap();
        let dir = Utf8Path::from_path(dir.path()).unwrap();
        // A runner with no commands at all, so every framed command prints its own failure.
        let ssh = Ssh::stand_in(
            dir,
            "#!/bin/sh\nfor command; do :; done\nPATH=/nonexistent exec /bin/sh -c \"$command\"\n",
        )
        .unwrap();
        let target = Target {
            label: "runner".to_owned(),
            ssh,
            desired: Desired::new(UpdateChannel::Stable, None, None, None),
        };
        let (_, frames) = target.collect().unwrap();
        assert_eq!(
            frames.keys().cloned().collect::<BTreeSet<_>>(),
            target
                .desired
                .frames()
                .map(|(name, _)| name)
                .collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn the_audit_counts_each_desired_state_difference() {
        let dir = tempfile::tempdir().unwrap();
        let dir = Utf8Path::from_path(dir.path()).unwrap();
        let (audit, rows) = absent_rows(dir);
        assert_eq!(
            audit
                .exec_with_fleet(&Fleet::default())
                .unwrap_err()
                .to_string(),
            format!(
                "Audit found 0 failed health check(s), {rows} desired state difference(s), and 0 differing section(s)"
            )
        );
    }

    #[test]
    fn the_audit_counts_scrub_days_too_close_as_a_failed_check() {
        let dir = tempfile::tempdir().unwrap();
        let dir = Utf8Path::from_path(dir.path()).unwrap();
        let (audit, rows) = absent_rows(dir);
        let day = |day| Some(ScrubDay::try_from(day).unwrap());
        let fleet = Fleet {
            scrub_days: vec![
                ("runner".parse().unwrap(), day(5)),
                ("other".parse().unwrap(), day(6)),
            ],
            ..Fleet::default()
        };
        assert_eq!(
            audit.exec_with_fleet(&fleet).unwrap_err().to_string(),
            format!(
                "Audit found 1 failed health check(s), {rows} desired state difference(s), and 0 differing section(s)"
            )
        );
    }

    #[test]
    fn the_audit_counts_refresh_minutes_too_close_as_a_failed_check() {
        // Kills a refresh minute check the audit prints but does not count.
        let dir = tempfile::tempdir().unwrap();
        let dir = Utf8Path::from_path(dir.path()).unwrap();
        let (audit, rows) = absent_rows(dir);
        let minute = |minute| Some(RefreshMinute::try_from(minute).unwrap());
        let fleet = Fleet {
            refresh_minutes: vec![
                ("runner".parse().unwrap(), minute(10)),
                ("other".parse().unwrap(), minute(12)),
            ],
            ..Fleet::default()
        };
        assert_eq!(
            audit.exec_with_fleet(&fleet).unwrap_err().to_string(),
            format!(
                "Audit found 1 failed health check(s), {rows} desired state difference(s), and 0 differing section(s)"
            )
        );
    }

    #[test]
    fn the_audit_counts_a_runner_with_no_image_cache_gib_as_a_failed_check() {
        // Kills an image cache check the audit prints but does not count, and
        // one that fails a fleet where every runner sets it.
        for (other, failed) in [(Some(gib(0)), 0), (None, 1)] {
            let dir = tempfile::tempdir().unwrap();
            let dir = Utf8Path::from_path(dir.path()).unwrap();
            let (audit, rows) = absent_rows(dir);
            let fleet = Fleet {
                image_cache_gibs: vec![
                    ("runner".parse().unwrap(), Some(gib(64))),
                    ("other".parse().unwrap(), other),
                ],
                ..Fleet::default()
            };
            assert_eq!(
                audit.exec_with_fleet(&fleet).unwrap_err().to_string(),
                format!(
                    "Audit found {failed} failed health check(s), {rows} desired state difference(s), and 0 differing section(s)"
                )
            );
        }
    }

    /// The audit of a healthy runner on which every desired state row is absent, and its number of rows.
    fn absent_rows(dir: &Utf8Path) -> (Audit, usize) {
        let desired = Desired::new(UpdateChannel::Stable, None, None, None);
        let rows = desired.frames().count();
        let mut reply = fake_output(&health::healthy());
        for (name, _) in desired.frames() {
            reply.push_str(&framed::fake_frame(&name, "absent", Some(0)));
        }
        fs::write(dir.join("reply"), reply).unwrap();
        let ssh = Ssh::stand_in(dir, &format!("#!/bin/sh\ncat {dir}/reply\n")).unwrap();
        let audit = Audit {
            runner: Target {
                label: "runner".to_owned(),
                ssh,
                desired,
            },
            against: None,
        };
        (audit, rows)
    }
}
