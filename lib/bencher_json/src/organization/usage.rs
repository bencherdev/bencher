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
    pub organization: OrganizationUuid,
    pub kind: UsageKind,
    pub plan: Option<JsonPlan>,
    pub license: Option<JsonLicense>,
    pub start_time: DateTime,
    pub end_time: DateTime,
    pub usage: Option<u32>,
}

#[typeshare::typeshare]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UsageKind {
    CloudFree,
    SelfHostedFree,
    CloudMetered,
    CloudLicensed,
    SelfHostedLicensedCloud,
    SelfHostedLicensed,
}
