use bencher_json::{
    project::threshold::{JsonNewStatistic, JsonThreshold, JsonThresholdStatistic},
    StatisticUuid, ThresholdUuid,
};
use chrono::Utc;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

use self::statistic::{InsertStatistic, QueryStatistic, StatisticId};
use super::{
    branch::{BranchId, QueryBranch},
    metric_kind::{MetricKindId, QueryMetricKind},
    testbed::{QueryTestbed, TestbedId},
    ProjectId, ProjectUuid, QueryProject,
};
use crate::{
    context::DbConnection,
    schema::threshold as threshold_table,
    schema::{self},
    util::{
        query::{fn_get, fn_get_id, fn_get_uuid},
        to_date_time,
    },
    ApiError,
};

pub mod alert;
pub mod boundary;
pub mod statistic;

crate::util::typed_id::typed_id!(ThresholdId);

#[derive(diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable)]
#[diesel(table_name = threshold_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
pub struct QueryThreshold {
    pub id: ThresholdId,
    pub uuid: ThresholdUuid,
    pub project_id: ProjectId,
    pub metric_kind_id: MetricKindId,
    pub branch_id: BranchId,
    pub testbed_id: TestbedId,
    pub statistic_id: Option<StatisticId>,
    pub created: i64,
    pub modified: i64,
}

impl QueryThreshold {
    fn_get!(threshold);
    fn_get_id!(threshold, ThresholdId);
    fn_get_uuid!(threshold, ThresholdId, ThresholdUuid);

    pub fn get_with_statistic(
        conn: &mut DbConnection,
        threshold_id: ThresholdId,
        statistic_id: StatisticId,
    ) -> Result<Self, ApiError> {
        let mut threshold = Self::get(conn, threshold_id)?;
        // IMPORTANT: Set the statistic ID to the one specified and not the current value!
        threshold.statistic_id = Some(statistic_id);
        Ok(threshold)
    }

    pub fn get_json(
        conn: &mut DbConnection,
        threshold_id: ThresholdId,
        statistic_id: StatisticId,
    ) -> Result<JsonThreshold, ApiError> {
        Self::get_with_statistic(conn, threshold_id, statistic_id)?.into_json(conn)
    }

    pub fn get_threshold_statistic_json(
        conn: &mut DbConnection,
        threshold_id: ThresholdId,
        statistic_id: StatisticId,
    ) -> Result<JsonThresholdStatistic, ApiError> {
        Self::get_with_statistic(conn, threshold_id, statistic_id)?
            .into_threshold_statistic_json(conn)
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonThreshold, ApiError> {
        let Self {
            uuid,
            project_id,
            metric_kind_id,
            branch_id,
            testbed_id,
            statistic_id,
            created,
            modified,
            ..
        } = self;
        Ok(JsonThreshold {
            uuid,
            project: QueryProject::get_uuid(conn, project_id)?,
            metric_kind: QueryMetricKind::get(conn, metric_kind_id)?.into_json(conn)?,
            branch: QueryBranch::get(conn, branch_id)?.into_json(conn)?,
            testbed: QueryTestbed::get(conn, testbed_id)?.into_json(conn)?,
            statistic: if let Some(statistic_id) = statistic_id {
                QueryStatistic::get(conn, statistic_id)?.into_json(conn)?
            } else {
                return Err(ApiError::NoThresholdStatistic(uuid));
            },
            created: to_date_time(created).map_err(ApiError::from)?,
            modified: to_date_time(modified).map_err(ApiError::from)?,
        })
    }

    pub fn into_threshold_statistic_json(
        self,
        conn: &mut DbConnection,
    ) -> Result<JsonThresholdStatistic, ApiError> {
        let project_uuid = QueryProject::get_uuid(conn, self.project_id)?;
        let statistic = if let Some(statistic_id) = self.statistic_id {
            QueryStatistic::get(conn, statistic_id)?
        } else {
            return Err(ApiError::NoThresholdStatistic(self.uuid));
        };
        self.into_threshold_statistic_json_for_project(project_uuid, statistic)
    }

    pub fn into_threshold_statistic_json_for_project(
        self,
        project_uuid: ProjectUuid,
        statistic: QueryStatistic,
    ) -> Result<JsonThresholdStatistic, ApiError> {
        let Self { uuid, created, .. } = self;
        Ok(JsonThresholdStatistic {
            uuid,
            project: project_uuid,
            statistic: statistic.into_json_for_threshold(uuid)?,
            created: to_date_time(created).map_err(ApiError::from)?,
        })
    }
}

#[derive(diesel::Insertable)]
#[diesel(table_name = threshold_table)]
pub struct InsertThreshold {
    pub uuid: ThresholdUuid,
    pub project_id: ProjectId,
    pub metric_kind_id: MetricKindId,
    pub branch_id: BranchId,
    pub testbed_id: TestbedId,
    pub statistic_id: Option<StatisticId>,
    pub created: i64,
    pub modified: i64,
}

impl InsertThreshold {
    pub fn new(
        project_id: ProjectId,
        metric_kind_id: MetricKindId,
        branch_id: BranchId,
        testbed_id: TestbedId,
    ) -> Self {
        let timestamp = Utc::now().timestamp();
        Self {
            uuid: ThresholdUuid::new(),
            project_id,
            metric_kind_id,
            branch_id,
            testbed_id,
            statistic_id: None,
            created: timestamp,
            modified: timestamp,
        }
    }

    pub fn from_json(
        conn: &mut DbConnection,
        project_id: ProjectId,
        metric_kind_id: MetricKindId,
        branch_id: BranchId,
        testbed_id: TestbedId,
        json_statistic: JsonNewStatistic,
    ) -> Result<ThresholdId, ApiError> {
        // Create the new threshold
        let insert_threshold =
            InsertThreshold::new(project_id, metric_kind_id, branch_id, testbed_id);
        diesel::insert_into(schema::threshold::table)
            .values(&insert_threshold)
            .execute(conn)
            .map_err(ApiError::from)?;

        // Get the new threshold ID
        let threshold_id = QueryThreshold::get_id(conn, &insert_threshold.uuid)?;

        // Create the new statistic
        let insert_statistic = InsertStatistic::from_json(threshold_id, json_statistic)?;
        diesel::insert_into(schema::statistic::table)
            .values(&insert_statistic)
            .execute(conn)
            .map_err(ApiError::from)?;

        // Get the new statistic ID
        let statistic_id = QueryStatistic::get_id(conn, &insert_statistic.uuid)?;

        // Set the new statistic for the new threshold
        diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(threshold_id)))
            .set(schema::threshold::statistic_id.eq(statistic_id))
            .execute(conn)
            .map_err(ApiError::from)?;

        Ok(threshold_id)
    }

    pub fn lower_boundary(
        conn: &mut DbConnection,
        project_id: ProjectId,
        metric_kind_id: MetricKindId,
        branch_id: BranchId,
        testbed_id: TestbedId,
    ) -> Result<ThresholdId, ApiError> {
        Self::from_json(
            conn,
            project_id,
            metric_kind_id,
            branch_id,
            testbed_id,
            JsonNewStatistic::lower_boundary(),
        )
    }

    pub fn upper_boundary(
        conn: &mut DbConnection,
        project_id: ProjectId,
        metric_kind_id: MetricKindId,
        branch_id: BranchId,
        testbed_id: TestbedId,
    ) -> Result<ThresholdId, ApiError> {
        Self::from_json(
            conn,
            project_id,
            metric_kind_id,
            branch_id,
            testbed_id,
            JsonNewStatistic::upper_boundary(),
        )
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = threshold_table)]
pub struct UpdateThreshold {
    pub statistic_id: StatisticId,
    pub modified: i64,
}

impl UpdateThreshold {
    pub fn new_statistic(
        conn: &mut DbConnection,
        statistic_uuid: StatisticUuid,
    ) -> Result<Self, ApiError> {
        Ok(Self {
            statistic_id: QueryStatistic::get_id(conn, &statistic_uuid)?,
            modified: Utc::now().timestamp(),
        })
    }
}
