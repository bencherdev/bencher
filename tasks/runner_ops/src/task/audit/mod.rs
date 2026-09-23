mod health;
mod snapshot;

use bencher_json::RunnerResourceId;

use super::merge_ssh;
use super::ssh::Ssh;
use crate::parser::TaskAudit;
use crate::parser::server::load_server;
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
        let (server, ssh, user) = merge_ssh(file.as_ref(), server, ssh, user)?;
        let label = runner.map_or_else(|| server.clone(), |runner| runner.to_string());
        Ok(Self {
            runner: Target {
                label,
                ssh: Ssh::new(server, ssh, user),
            },
            against: against.as_ref().map(Target::from_file).transpose()?,
        })
    }
}

impl Audit {
    pub fn exec(self) -> anyhow::Result<()> {
        let Self { runner, against } = self;
        let sections = runner.collect()?;
        let snapshot = Snapshot::new(&sections);
        report_size(&runner.label, &snapshot);

        let mut differing = 0;
        let reference = if let Some(reference) = against {
            let reference_sections = reference.collect()?;
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
            Some((reference, reference_sections))
        } else {
            print!("{snapshot}");
            None
        };

        let mut failed = report_health(&runner.label, &sections);
        if let Some((reference, reference_sections)) = &reference {
            failed += report_health(&reference.label, reference_sections);
        }

        if failed > 0 || differing > 0 {
            anyhow::bail!(
                "Audit found {failed} failed health check(s) and {differing} differing section(s)"
            );
        }
        println!("Audit clean");
        Ok(())
    }
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
        })
    }

    fn collect(&self) -> anyhow::Result<Sections> {
        println!("Collecting an audit snapshot from {}...", self.label);
        let output = self.ssh.run_quiet(&snapshot::script())?;
        Ok(Sections::parse(&output))
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

fn report_size(label: &str, snapshot: &Snapshot) {
    let (sections, lines) = snapshot.size();
    println!("Audited {sections} sections ({lines} lines) from {label}");
}
