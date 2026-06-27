#![cfg(feature = "plus")]

use bencher_valid::DateTime;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{JsonPlan, OrganizationUuid};

use super::plan::JsonLicense;

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUsage {
    /// The organization UUID.
    pub organization: OrganizationUuid,
    /// The kind of usage.
    pub kind: UsageKind,
    /// The organization plan.
    pub plan: Option<JsonPlan>,
    /// The organization license.
    pub license: Option<JsonLicense>,
    /// The start time of the usage.
    pub start_time: DateTime,
    /// The end time of the usage.
    pub end_time: DateTime,
    /// The metrics usage amount. Not populated for Pro metered plans, which bill on
    /// active series (see `active_series`) rather than metrics.
    pub metrics: Option<u32>,
    /// The active series usage amount. Populated for Pro metered plans, whose bill is
    /// based on monthly-active series (distinct testbed x benchmark x measure).
    pub active_series: Option<u32>,
    /// The runner minutes usage amount.
    pub runner_minutes: Option<u32>,
}

#[typeshare::typeshare]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UsageKind {
    /// Bencher Cloud (Free)
    CloudFree,
    /// Bencher Cloud (Metered)
    CloudMetered,
    /// Bencher Self-Hosted (Licensed) via Bencher Cloud
    CloudSelfHostedLicensed,
    /// Bencher Self-Hosted (Free)
    SelfHostedFree,
    /// Bencher Self-Hosted (Licensed)
    SelfHostedLicensed,
}
