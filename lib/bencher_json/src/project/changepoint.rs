#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    BenchmarkNameId, BranchNameId, DateTime, JsonBenchmark, JsonThreshold, JsonVariant,
    MeasureNameId, MetricName, TestbedNameId,
    urlencoded::{UrlEncodedError, from_urlencoded, to_urlencoded},
};

crate::typed_uuid::typed_uuid!(ChangepointUuid);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonChangepoints(pub Vec<JsonChangepoint>);

crate::from_vec!(JsonChangepoints[JsonChangepoint]);

/// A version at which a line stepped, found after the fact by a threshold's changepoint model.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonChangepoint {
    pub uuid: ChangepointUuid,
    /// The threshold that found the changepoint.
    ///
    /// Its branch is given at the head and the version the changepoint is on,
    /// and its `changepoint_model` is the model the changepoint was judged against.
    pub threshold: JsonThreshold,
    pub benchmark: JsonBenchmark,
    /// The variant of the benchmark the line is on.
    pub variant: JsonVariant,
    /// The name of the metrics the line is made of.
    pub metric: MetricName,
    /// The mean of the points before the changepoint.
    pub mean_before: f64,
    /// The mean of the points from the changepoint on.
    pub mean_after: f64,
    /// The sample standard deviation of the points before the changepoint.
    pub stddev_before: f64,
    /// The sample standard deviation of the points from the changepoint on.
    pub stddev_after: f64,
    /// The two-sided p value of Student's t-test on the two sides.
    pub pvalue: f64,
    /// The t statistic of that test, positive when the mean before is the larger.
    /// Absent when it is infinite, which it is when both sides are constant at different values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tstatistic: Option<f64>,
    /// The number of points on the two sides less 2.
    pub degrees_of_freedom: u32,
    /// The change from the mean before to the mean after, as a percent of the mean before.
    /// It is 0 when the mean before is 0.
    /// Absent when it is not finite, which it is when the mean before is too close to 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent_change: Option<f64>,
    /// The scans that found the changepoint, oldest first, up to the one that confirmed it.
    /// While a changepoint is active its measured fields are its confirming scan's.
    /// Served only by the view of one changepoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scans: Option<Vec<JsonChangepointScan>>,
    pub status: ChangepointStatus,
    pub created: DateTime,
    pub modified: DateTime,
}

/// What one scan that found a changepoint measured.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonChangepointScan {
    /// The mean of the points before the changepoint.
    pub mean_before: f64,
    /// The mean of the points from the changepoint on.
    pub mean_after: f64,
    /// The sample standard deviation of the points before the changepoint.
    pub stddev_before: f64,
    /// The sample standard deviation of the points from the changepoint on.
    pub stddev_after: f64,
    /// The two-sided p value of Student's t-test on the two sides.
    pub pvalue: f64,
    /// The t statistic of that test, positive when the mean before is the larger.
    /// Absent when it is infinite, which it is when both sides are constant at different values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tstatistic: Option<f64>,
    /// The number of points on the two sides less 2.
    pub degrees_of_freedom: u32,
    /// The change from the mean before to the mean after, as a percent of the mean before.
    /// It is 0 when the mean before is 0.
    /// Absent when it is not finite, which it is when the mean before is too close to 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent_change: Option<f64>,
    /// When the scan ran.
    pub created: DateTime,
}

/// The status of a changepoint a user can see.
#[typeshare::typeshare]
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, derive_more::Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum ChangepointStatus {
    #[default]
    /// The changepoint is active.
    Active,
    /// The changepoint has been dismissed by a user.
    Dismissed,
    /// The changepoint has been silenced, because the head it is on was replaced.
    Silenced,
    /// The changepoint has been withdrawn, because the step it found moved or vanished.
    Withdrawn,
}

impl From<UpdateChangepointStatus> for ChangepointStatus {
    fn from(status: UpdateChangepointStatus) -> Self {
        match status {
            UpdateChangepointStatus::Active => Self::Active,
            UpdateChangepointStatus::Dismissed => Self::Dismissed,
        }
    }
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateChangepoint {
    /// The new status of the changepoint.
    pub status: UpdateChangepointStatus,
}

/// The statuses a user may move a changepoint between.
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum UpdateChangepointStatus {
    /// The changepoint is active.
    Active,
    /// The changepoint has been dismissed by a user.
    Dismissed,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonChangepointQueryParams {
    /// Filter by changepoint status.
    /// Defaults to `active`.
    pub status: Option<ChangepointStatus>,
    /// Filter by branch name, slug, or UUID, exact match.
    pub branch: Option<String>,
    /// Filter by testbed name, slug, or UUID, exact match.
    pub testbed: Option<String>,
    /// Filter by benchmark name, slug, or UUID, exact match.
    pub benchmark: Option<String>,
    /// Filter by measure name, slug, or UUID, exact match.
    pub measure: Option<String>,
    /// If set to `true`, only returns changepoints with an archived branch, testbed, or measure.
    /// If not set or set to `false`, only returns changepoints with non-archived branches, testbeds, and measures.
    pub archived: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct JsonChangepointQuery {
    pub status: Option<ChangepointStatus>,
    pub branch: Option<BranchNameId>,
    pub testbed: Option<TestbedNameId>,
    pub benchmark: Option<BenchmarkNameId>,
    pub measure: Option<MeasureNameId>,
    pub archived: Option<bool>,
}

impl TryFrom<JsonChangepointQueryParams> for JsonChangepointQuery {
    type Error = UrlEncodedError;

    fn try_from(query_params: JsonChangepointQueryParams) -> Result<Self, Self::Error> {
        let JsonChangepointQueryParams {
            status,
            branch,
            testbed,
            benchmark,
            measure,
            archived,
        } = query_params;
        Ok(Self {
            status,
            branch: branch.as_deref().map(from_urlencoded).transpose()?,
            testbed: testbed.as_deref().map(from_urlencoded).transpose()?,
            benchmark: benchmark.as_deref().map(from_urlencoded).transpose()?,
            measure: measure.as_deref().map(from_urlencoded).transpose()?,
            archived,
        })
    }
}

impl JsonChangepointQuery {
    pub fn branch(&self) -> Option<String> {
        self.branch.as_ref().map(to_urlencoded)
    }

    pub fn testbed(&self) -> Option<String> {
        self.testbed.as_ref().map(to_urlencoded)
    }

    pub fn benchmark(&self) -> Option<String> {
        self.benchmark.as_ref().map(to_urlencoded)
    }

    pub fn measure(&self) -> Option<String> {
        self.measure.as_ref().map(to_urlencoded)
    }
}

/// A count a changepoint keeps: the degrees of freedom of its test.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, derive_more::Display)]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
pub struct ChangepointCount(pub u32);

impl From<ChangepointCount> for u32 {
    fn from(count: ChangepointCount) -> Self {
        count.0
    }
}

#[cfg(feature = "db")]
mod changepoint_count {
    use super::ChangepointCount;

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for ChangepointCount
    where
        DB: diesel::backend::Backend,
        for<'a> i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>
            + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            out.set_value(i32::try_from(self.0)?);
            Ok(diesel::serialize::IsNull::No)
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for ChangepointCount
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            Ok(Self(u32::try_from(i32::from_sql(bytes)?)?))
        }
    }
}
