#![cfg(feature = "plus")]

use bencher_json::{
    BenchmarkNameId, BranchNameId, ChangepointUuid, MeasureNameId, ProjectResourceId, TestbedNameId,
};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::{CliBackend, CliPagination};

#[derive(Subcommand, Debug)]
pub enum CliChangepoint {
    /// List changepoints
    #[clap(alias = "ls")]
    List(CliChangepointList),
    /// View a changepoint
    #[clap(alias = "get")]
    View(CliChangepointView),
    /// Update a changepoint
    #[clap(alias = "edit")]
    Update(CliChangepointUpdate),
}

#[derive(Parser, Debug)]
pub struct CliChangepointList {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    #[clap(flatten)]
    pub pagination: CliPagination<CliChangepointsSort>,

    /// Filter by changepoint status, active by default
    #[clap(long)]
    pub status: Option<CliChangepointStatus>,

    /// Branch name, slug, or UUID
    #[clap(long)]
    pub branch: Option<BranchNameId>,

    /// Testbed name, slug, or UUID
    #[clap(long)]
    pub testbed: Option<TestbedNameId>,

    /// Benchmark name, slug, or UUID
    #[clap(long)]
    pub benchmark: Option<BenchmarkNameId>,

    /// Measure name, slug, or UUID
    #[clap(long)]
    pub measure: Option<MeasureNameId>,

    /// Filter for changepoints with an archived branch, testbed, or measure
    #[clap(long)]
    pub archived: bool,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
#[clap(rename_all = "snake_case")]
pub enum CliChangepointsSort {
    /// Creation date time of the changepoint
    Created,
    /// Modification date time of the changepoint
    Modified,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
#[clap(rename_all = "snake_case")]
pub enum CliChangepointStatus {
    /// Active
    Active,
    /// Dismissed
    Dismissed,
    /// Silenced
    Silenced,
    /// Withdrawn
    Withdrawn,
}

#[derive(Parser, Debug)]
pub struct CliChangepointView {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Changepoint UUID
    pub changepoint: ChangepointUuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliChangepointUpdate {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Changepoint UUID
    pub changepoint: ChangepointUuid,

    /// Changepoint status
    #[clap(long)]
    pub status: CliChangepointStatusUpdate,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
#[clap(rename_all = "snake_case")]
pub enum CliChangepointStatusUpdate {
    /// Active
    Active,
    /// Dismissed
    Dismissed,
}

#[cfg(test)]
mod tests {
    use bencher_json::{
        BenchmarkNameId, BranchNameId, ChangepointUuid, MeasureNameId, TestbedNameId,
    };
    use clap::Parser as _;

    use super::{
        CliChangepoint, CliChangepointStatus, CliChangepointStatusUpdate, CliChangepointsSort,
    };
    use crate::parser::{CliBencher, CliSub};

    const UUID: &str = "00000000-0000-0000-0000-000000000001";

    fn parse(args: &[&str]) -> Result<CliChangepoint, String> {
        let cli = CliBencher::try_parse_from(["bencher", "changepoint"].iter().chain(args))
            .map_err(|error| error.to_string())?;
        if let CliSub::Changepoint(changepoint) = cli.sub {
            Ok(changepoint)
        } else {
            Err("not a changepoint command".to_owned())
        }
    }

    // `bencher changepoint list` takes a status and each dimension the list filters by.
    #[test]
    fn list_takes_a_status_and_the_dimensions() {
        let Ok(CliChangepoint::List(list)) = parse(&[
            "list",
            "my-project",
            "--status",
            "withdrawn",
            "--branch",
            "main",
            "--testbed",
            "localhost",
            "--benchmark",
            "bench",
            "--measure",
            "latency",
            "--archived",
            "--sort",
            "created",
        ]) else {
            panic!("not a changepoint list");
        };
        assert_eq!(list.status, Some(CliChangepointStatus::Withdrawn));
        assert_eq!(list.branch, Some("main".parse::<BranchNameId>().unwrap()));
        assert_eq!(
            list.testbed,
            Some("localhost".parse::<TestbedNameId>().unwrap())
        );
        assert_eq!(
            list.benchmark,
            Some("bench".parse::<BenchmarkNameId>().unwrap())
        );
        assert_eq!(
            list.measure,
            Some("latency".parse::<MeasureNameId>().unwrap())
        );
        assert!(list.archived);
        assert!(matches!(
            list.pagination.sort,
            Some(CliChangepointsSort::Created)
        ));
    }

    // A list filters by a status a user sees, never by a candidate.
    #[test]
    fn list_refuses_a_candidate_status() {
        parse(&["list", "my-project", "--status", "candidate"]).unwrap_err();
    }

    // `bencher changepoint view` takes the changepoint's UUID.
    #[test]
    fn view_takes_a_changepoint_uuid() {
        let Ok(CliChangepoint::View(view)) = parse(&["view", "my-project", UUID]) else {
            panic!("not a changepoint view");
        };
        assert_eq!(view.changepoint, UUID.parse::<ChangepointUuid>().unwrap());
        parse(&["view", "my-project", "not-a-uuid"]).unwrap_err();
    }

    // `bencher changepoint update` needs a status, and a user may only make a changepoint active or
    // dismissed.
    #[test]
    fn update_takes_active_or_dismissed() {
        for (status, expected) in [
            ("active", CliChangepointStatusUpdate::Active),
            ("dismissed", CliChangepointStatusUpdate::Dismissed),
        ] {
            let Ok(CliChangepoint::Update(update)) =
                parse(&["update", "my-project", UUID, "--status", status])
            else {
                panic!("not a changepoint update");
            };
            assert_eq!(update.status, expected);
        }
        for refused in [
            &["update", "my-project", UUID][..],
            &["update", "my-project", UUID, "--status", "silenced"],
            &["update", "my-project", UUID, "--status", "withdrawn"],
        ] {
            parse(refused).unwrap_err();
        }
    }
}
