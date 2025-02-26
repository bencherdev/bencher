use bencher_context::ReportContext;
use bencher_valid::{DateTime, GitHash, ResourceId};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    project::{
        branch::{JsonUpdateStartPoint, DEFAULT_BRANCH},
        report::{JsonReportSettings, JsonReportThresholds},
        testbed::DEFAULT_TESTBED,
    },
    JsonNewReport, NameId,
};

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewRun {
    /// Organization UUID or slug.
    /// If the organization is not provided, it will be created.
    pub organization: Option<ResourceId>,
    /// Project UUID, slug, or name.
    /// If the project is not provided or does not exist, it will be created.
    /// If a name is provided, the `organization` field must also be provided.
    pub project: Option<NameId>,
    /// Branch UUID, slug, or name.
    /// If the branch is not provided or does not exist, it will be created.
    pub branch: Option<NameId>,
    /// Full `git` commit hash.
    /// All reports with the same `git` commit hash will be considered part of the same branch version.
    /// This can be useful for tracking the performance of a specific commit across multiple testbeds.
    pub hash: Option<GitHash>,
    /// The start point for the report branch.
    /// If the branch does not exist, the start point will be used to create a new branch.
    /// If the branch already exists and the start point is not provided, the current branch will be used.
    /// If the branch already exists and the start point provided is different, a new branch head will be created from the new start point.
    /// If a new branch or new branch head is created with a start point,
    /// historical branch versions from the start point branch will be shallow copied over to the new branch.
    /// That is, historical metrics data for the start point branch will appear in queries for the branch.
    /// For example, pull request branches often use their base branch as their start point branch.
    /// If a new branch is created, it is not kept in sync with the start point branch.
    pub start_point: Option<JsonUpdateStartPoint>,
    /// Testbed UUID, slug, or name.
    /// If the testbed is not provided or does not exist, it will be created.
    pub testbed: Option<NameId>,
    /// Thresholds to use for the branch, testbed, and measures in the report.
    /// If a threshold does not exist, it will be created.
    /// If a threshold exists and the model is different, it will be updated with the new model.
    /// If a measure name or slug is provided, the measure will be created if it does not exist.
    pub thresholds: Option<JsonReportThresholds>,
    /// Start time for the report. Must be an ISO 8601 formatted string.
    pub start_time: DateTime,
    /// End time for the report. Must be an ISO 8601 formatted string.
    pub end_time: DateTime,
    /// An array of benchmarks results.
    pub results: Vec<String>,
    /// Settings for how to handle the results.
    pub settings: Option<JsonReportSettings>,
    /// Context for the report.
    pub context: Option<ReportContext>,
}

impl From<JsonNewRun> for JsonNewReport {
    fn from(run: JsonNewRun) -> Self {
        let JsonNewRun {
            organization: _,
            project: _,
            branch,
            hash,
            start_point,
            testbed,
            thresholds,
            start_time,
            end_time,
            results,
            settings,
            context,
        } = run;
        let branch = branch
            .or_else(|| {
                context
                    .as_ref()
                    .and_then(|ctx| ctx.branch_ref_name().and_then(|branch| branch.parse().ok()))
            })
            .unwrap_or_else(|| DEFAULT_BRANCH.clone());
        let hash = hash.or_else(|| {
            context
                .as_ref()
                .and_then(|ctx| ctx.branch_hash().and_then(|hash| hash.parse().ok()))
        });
        let testbed = testbed
            .or_else(|| {
                context
                    .as_ref()
                    .and_then(|ctx| ctx.testbed_os().and_then(|testbed| testbed.parse().ok()))
            })
            .unwrap_or_else(|| DEFAULT_TESTBED.clone());
        Self {
            branch,
            hash,
            start_point,
            testbed,
            thresholds,
            start_time,
            end_time,
            results,
            settings,
            context,
        }
    }
}
