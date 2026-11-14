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
    /// The metrics usage amount. Populated for the Free kinds only: every plan and license
    /// counts active series (see `active_series`) rather than metrics.
    pub metrics: Option<u32>,
    /// The active series usage amount. Populated for metered plans, whose bill is
    /// based on monthly-active series (distinct testbed x benchmark x variant x measure)
    /// in the billing period, and for a Self-Hosted license, which counts them over the
    /// last 30 days.
    pub active_series: Option<u32>,
    /// The highest count of active series the server has seen in the term of a valid
    /// Self-Hosted license. `None` until a report in the term has been counted.
    pub peak: Option<JsonLicensePeak>,
    /// The peak of the latest term before the term of a valid Self-Hosted license,
    /// or of the latest term when the organization holds no valid license.
    /// `None` when the server has counted no such term.
    pub previous_peak: Option<JsonLicensePeak>,
    /// The runner minutes usage amount.
    pub runner_minutes: Option<u32>,
}

/// The highest count of active series a Self-Hosted server has seen in one paid term of a
/// license.
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonLicensePeak {
    /// The end of the paid term.
    pub term_end: DateTime,
    /// The organization's Benchmark Series active in the 30 days before the count.
    pub active_series: u32,
    /// When the server counted them.
    pub seen: DateTime,
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
