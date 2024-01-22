use bencher_json::{
    project::threshold::{JsonThreshold, JsonThresholdStatistic},
    DateTime, Statistic, StatisticUuid, ThresholdUuid,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;
use http::StatusCode;

use self::statistic::{InsertStatistic, QueryStatistic, StatisticId};
use super::{
    branch::{BranchId, QueryBranch},
    measure::{MeasureId, QueryMeasure},
    testbed::{QueryTestbed, TestbedId},
    ProjectId, QueryProject,
};
use crate::{
    context::DbConnection,
    error::{assert_parentage, issue_error, resource_conflict_err, BencherResource},
    schema::threshold as threshold_table,
    schema::{self},
    util::fn_get::{fn_get, fn_get_id, fn_get_uuid},
};

pub mod alert;
pub mod boundary;
pub mod statistic;

crate::util::typed_id::typed_id!(ThresholdId);

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = threshold_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
pub struct QueryThreshold {
    pub id: ThresholdId,
    pub uuid: ThresholdUuid,
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub testbed_id: TestbedId,
    pub measure_id: MeasureId,
    pub statistic_id: Option<StatisticId>,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryThreshold {
    fn_get!(threshold, ThresholdId);
    fn_get_id!(threshold, ThresholdId, ThresholdUuid);
    fn_get_uuid!(threshold, ThresholdId, ThresholdUuid);

    pub fn get_with_statistic(
        conn: &mut DbConnection,
        threshold_id: ThresholdId,
        statistic_id: StatisticId,
    ) -> Result<Self, HttpError> {
        let mut threshold = Self::get(conn, threshold_id)?;
        // IMPORTANT: Set the statistic ID to the one specified and not the current value!
        threshold.statistic_id = Some(statistic_id);
        Ok(threshold)
    }

    pub fn get_json(
        conn: &mut DbConnection,
        threshold_id: ThresholdId,
        statistic_id: StatisticId,
    ) -> Result<JsonThreshold, HttpError> {
        Self::get_with_statistic(conn, threshold_id, statistic_id)?.into_json(conn)
    }

    pub fn get_threshold_statistic_json(
        conn: &mut DbConnection,
        threshold_id: ThresholdId,
        statistic_id: StatisticId,
    ) -> Result<JsonThresholdStatistic, HttpError> {
        Self::get_with_statistic(conn, threshold_id, statistic_id)?
            .into_threshold_statistic_json(conn)
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonThreshold, HttpError> {
        let Self {
            uuid,
            project_id,
            branch_id,
            testbed_id,
            measure_id,
            statistic_id,
            created,
            modified,
            ..
        } = self;
        let statistic = if let Some(statistic_id) = statistic_id {
            QueryStatistic::get(conn, statistic_id)?.into_json(conn)?
        } else {
            let err = issue_error(
                StatusCode::NOT_FOUND,
                "Failed to find statistic for threshold",
                &format!("No statistic for threshold: {project_id}/{uuid}"),
                "statistic is null",
            );
            debug_assert!(false, "{err}");
            #[cfg(feature = "sentry")]
            sentry::capture_error(&err);
            return Err(err);
        };
        Ok(JsonThreshold {
            uuid,
            project: QueryProject::get_uuid(conn, project_id)?,
            branch: QueryBranch::get(conn, branch_id)?.into_json(conn)?,
            testbed: QueryTestbed::get(conn, testbed_id)?.into_json(conn)?,
            measure: QueryMeasure::get(conn, measure_id)?.into_json(conn)?,
            statistic,
            created,
            modified,
        })
    }

    pub fn into_threshold_statistic_json(
        self,
        conn: &mut DbConnection,
    ) -> Result<JsonThresholdStatistic, HttpError> {
        let project = QueryProject::get(conn, self.project_id)?;
        let statistic = if let Some(statistic_id) = self.statistic_id {
            QueryStatistic::get(conn, statistic_id)?
        } else {
            let err = issue_error(
                StatusCode::NOT_FOUND,
                "Failed to find threshold statistic ",
                &format!("No threshold statistic: {self:?}"),
                "statistic is null",
            );
            debug_assert!(false, "{err}");
            #[cfg(feature = "sentry")]
            sentry::capture_error(&err);
            return Err(err);
        };
        Ok(self.into_threshold_statistic_json_for_project(&project, statistic))
    }

    pub fn into_threshold_statistic_json_for_project(
        self,
        project: &QueryProject,
        statistic: QueryStatistic,
    ) -> JsonThresholdStatistic {
        let statistic = statistic.into_json_for_threshold(&self);
        let Self {
            uuid,
            project_id,
            created,
            ..
        } = self;
        assert_parentage(
            BencherResource::Project,
            project.id,
            BencherResource::Threshold,
            project_id,
        );
        JsonThresholdStatistic {
            uuid,
            project: project.uuid,
            statistic,
            created,
        }
    }
}

#[derive(Debug, Clone, diesel::Insertable)]
#[diesel(table_name = threshold_table)]
pub struct InsertThreshold {
    pub uuid: ThresholdUuid,
    pub project_id: ProjectId,
    pub branch_id: BranchId,
    pub testbed_id: TestbedId,
    pub measure_id: MeasureId,
    pub statistic_id: Option<StatisticId>,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertThreshold {
    pub fn new(
        project_id: ProjectId,
        branch_id: BranchId,
        testbed_id: TestbedId,
        measure_id: MeasureId,
    ) -> Self {
        let timestamp = DateTime::now();
        Self {
            uuid: ThresholdUuid::new(),
            project_id,
            branch_id,
            testbed_id,
            measure_id,
            statistic_id: None,
            created: timestamp,
            modified: timestamp,
        }
    }

    pub fn insert_from_json(
        conn: &mut DbConnection,
        project_id: ProjectId,
        branch_id: BranchId,
        testbed_id: TestbedId,
        measure_id: MeasureId,
        statistic: Statistic,
    ) -> Result<ThresholdId, HttpError> {
        // Create the new threshold
        let insert_threshold = InsertThreshold::new(project_id, branch_id, testbed_id, measure_id);
        diesel::insert_into(schema::threshold::table)
            .values(&insert_threshold)
            .execute(conn)
            .map_err(resource_conflict_err!(Threshold, insert_threshold))?;

        // Get the new threshold ID
        let threshold_id = QueryThreshold::get_id(conn, insert_threshold.uuid)?;

        // Create the new statistic
        let insert_statistic = InsertStatistic::from_json(threshold_id, statistic);
        diesel::insert_into(schema::statistic::table)
            .values(&insert_statistic)
            .execute(conn)
            .map_err(resource_conflict_err!(Statistic, insert_statistic))?;

        // Get the new statistic ID
        let statistic_id = QueryStatistic::get_id(conn, insert_statistic.uuid)?;

        // Set the new statistic for the new threshold
        diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(threshold_id)))
            .set(schema::threshold::statistic_id.eq(statistic_id))
            .execute(conn)
            .map_err(resource_conflict_err!(
                Threshold,
                (threshold_id, &insert_statistic)
            ))?;

        Ok(threshold_id)
    }

    pub fn lower_boundary(
        conn: &mut DbConnection,
        project_id: ProjectId,
        branch_id: BranchId,
        testbed_id: TestbedId,
        measure_id: MeasureId,
    ) -> Result<ThresholdId, HttpError> {
        Self::insert_from_json(
            conn,
            project_id,
            branch_id,
            testbed_id,
            measure_id,
            Statistic::lower_boundary(),
        )
    }

    pub fn upper_boundary(
        conn: &mut DbConnection,
        project_id: ProjectId,
        branch_id: BranchId,
        testbed_id: TestbedId,
        measure_id: MeasureId,
    ) -> Result<ThresholdId, HttpError> {
        Self::insert_from_json(
            conn,
            project_id,
            branch_id,
            testbed_id,
            measure_id,
            Statistic::upper_boundary(),
        )
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = threshold_table)]
pub struct UpdateThreshold {
    pub statistic_id: StatisticId,
    pub modified: DateTime,
}

impl UpdateThreshold {
    pub fn new_statistic(
        conn: &mut DbConnection,
        statistic_uuid: StatisticUuid,
    ) -> Result<Self, HttpError> {
        Ok(Self {
            statistic_id: QueryStatistic::get_id(conn, statistic_uuid)?,
            modified: DateTime::now(),
        })
    }
}
