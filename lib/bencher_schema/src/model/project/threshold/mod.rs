use std::collections::HashMap;

use bencher_json::{
    DateTime, Model, ModelUuid, ThresholdUuid,
    project::{
        report::JsonReportThresholds,
        threshold::{JsonThreshold, JsonThresholdModel},
    },
};
use diesel::{BelongingToDsl as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;
use model::UpdateModel;
use slog::Logger;

use self::model::{InsertModel, ModelId, QueryModel};
use super::{
    ProjectId, QueryProject,
    branch::{BranchId, QueryBranch, head::HeadId, start_point::StartPoint, version::VersionId},
    measure::{MeasureId, QueryMeasure},
    testbed::{QueryTestbed, TestbedId},
};
use crate::{
    auth_conn,
    context::{ApiContext, DbConnection},
    error::{
        BencherResource, assert_parentage, assert_siblings, resource_conflict_err,
        resource_not_found_err,
    },
    macros::fn_get::{fn_get, fn_get_id, fn_get_uuid},
    schema::{self, threshold as threshold_table},
    write_conn,
};

pub mod alert;
pub mod boundary;
pub mod model;

crate::macros::typed_id::typed_id!(ThresholdId);

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
    pub model_id: Option<ModelId>,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryThreshold {
    fn_get!(threshold, ThresholdId);
    fn_get_id!(threshold, ThresholdId, ThresholdUuid);
    fn_get_uuid!(threshold, ThresholdId, ThresholdUuid);

    pub fn get_with_uuid(
        conn: &mut DbConnection,
        query_project: &QueryProject,
        uuid: ThresholdUuid,
    ) -> Result<Self, HttpError> {
        Self::belonging_to(&query_project)
            .filter(threshold_table::uuid.eq(uuid))
            .first::<Self>(conn)
            .map_err(resource_not_found_err!(Threshold, (query_project, uuid)))
    }

    pub fn model(&self, conn: &mut DbConnection) -> Result<Option<QueryModel>, HttpError> {
        if let Some(model_id) = self.model_id {
            Ok(Some(QueryModel::get(conn, model_id)?))
        } else {
            Ok(None)
        }
    }

    pub async fn update_model_if_changed(
        &self,
        context: &ApiContext,
        model: Option<Model>,
    ) -> Result<(), HttpError> {
        match (self.model_id, model) {
            // No current model and no new model,
            // nothing to do.
            (None, None) => Ok(()),
            // No current model but a new model,
            // insert the new model.
            (None, Some(model)) => self.update_from_model(context, model).await,
            // Current model but no new model,
            // remove the current model.
            (Some(_), None) => self.remove_current_model(write_conn!(context)),
            // Current model and new model,
            // update the current if it has changed.
            (Some(model_id), Some(model)) => {
                let current_model = QueryModel::get(auth_conn!(context), model_id)?.into_model();
                // Skip updating the model if it has not changed.
                // This keeps us from needlessly replacing old models with identical new ones.
                if current_model == model {
                    Ok(())
                } else {
                    self.update_from_model(context, model).await
                }
            },
        }
    }

    async fn update_from_model(&self, context: &ApiContext, model: Model) -> Result<(), HttpError> {
        #[cfg(feature = "plus")]
        InsertModel::rate_limit(context, self).await?;
        self.update_from_model_inner(write_conn!(context), model)
    }

    fn update_from_model_inner(
        &self,
        conn: &mut DbConnection,
        model: Model,
    ) -> Result<(), HttpError> {
        // Insert the new model
        let insert_model = InsertModel::new(self.id, model);
        diesel::insert_into(schema::model::table)
            .values(&insert_model)
            .execute(conn)
            .map_err(resource_conflict_err!(Model, (self, &insert_model)))?;

        // Update the current threshold to use the new model
        let update_threshold = UpdateThreshold::new_model(conn, insert_model.uuid)?;
        diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(self.id)))
            .set(&update_threshold)
            .execute(conn)
            .map_err(resource_conflict_err!(
                Threshold,
                (&self, &insert_model, &update_threshold)
            ))?;

        self.update_replaced_model(conn, update_threshold.modified)
    }

    pub fn remove_current_model(&self, conn: &mut DbConnection) -> Result<(), HttpError> {
        // Skip if there is no current model
        if self.model_id.is_none() {
            return Ok(());
        }

        // Update the current threshold to remove the new model
        let update_threshold = UpdateThreshold::remove_model();
        diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(self.id)))
            .set(&update_threshold)
            .execute(conn)
            .map_err(resource_conflict_err!(
                Threshold,
                (&self, &update_threshold)
            ))?;

        self.update_replaced_model(conn, update_threshold.modified)
    }

    pub fn update_replaced_model(
        &self,
        conn: &mut DbConnection,
        date_time: DateTime,
    ) -> Result<(), HttpError> {
        // Update the old model to be replaced, if there is one
        if let Some(model_id) = self.model_id {
            let update_model = UpdateModel::replaced_at(date_time);
            diesel::update(schema::model::table.filter(schema::model::id.eq(model_id)))
                .set(&update_model)
                .execute(conn)
                .map_err(resource_conflict_err!(Model, (&self, &update_model)))?;
        }
        Ok(())
    }

    pub fn get_alert_json(
        conn: &mut DbConnection,
        threshold_id: ThresholdId,
        model_id: ModelId,
        head_id: HeadId,
        version_id: VersionId,
    ) -> Result<JsonThreshold, HttpError> {
        let query_threshold = Self::get(conn, threshold_id)?;
        let query_model = QueryModel::get(conn, model_id)?;
        query_threshold.into_json_for_model(conn, Some(query_model), Some((head_id, version_id)))
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonThreshold, HttpError> {
        let query_model = self.model(conn)?;
        self.into_json_for_model(conn, query_model, None)
    }

    pub fn into_json_for_model(
        self,
        conn: &mut DbConnection,
        query_model: Option<QueryModel>,
        head_version: Option<(HeadId, VersionId)>,
    ) -> Result<JsonThreshold, HttpError> {
        let model = if let Some(query_model) = &query_model {
            assert_parentage(
                BencherResource::Threshold,
                self.id,
                BencherResource::Model,
                query_model.threshold_id,
            );
            Some(query_model.into_json(&self))
        } else {
            None
        };
        let Self {
            uuid,
            project_id,
            branch_id,
            testbed_id,
            measure_id,
            created,
            modified,
            ..
        } = self;
        let query_project = QueryProject::get(conn, project_id)?;
        let branch = if let Some((head_id, version_id)) = head_version {
            QueryBranch::get_json_for_report(conn, &query_project, head_id, version_id)?
        } else {
            let query_branch = QueryBranch::get(conn, branch_id)?;
            query_branch.into_json_for_project(conn, &query_project)?
        };
        let testbed = QueryTestbed::get(conn, testbed_id)?.into_json_for_project(&query_project);
        let measure = QueryMeasure::get(conn, measure_id)?.into_json_for_project(&query_project);
        Ok(JsonThreshold {
            uuid,
            project: query_project.uuid,
            branch,
            testbed,
            measure,
            model,
            created,
            modified,
        })
    }

    pub fn into_threshold_model_json_for_project(
        self,
        project: &QueryProject,
        query_model: QueryModel,
    ) -> JsonThresholdModel {
        let model = query_model.into_json(&self);
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
        JsonThresholdModel {
            uuid,
            project: project.uuid,
            model,
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
    pub model_id: Option<ModelId>,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertThreshold {
    #[cfg(feature = "plus")]
    crate::macros::rate_limit::fn_rate_limit!(threshold, Threshold);

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
            model_id: None,
            created: timestamp,
            modified: timestamp,
        }
    }

    pub async fn from_model(
        context: &ApiContext,
        project_id: ProjectId,
        branch_id: BranchId,
        testbed_id: TestbedId,
        measure_id: MeasureId,
        model: Model,
    ) -> Result<ThresholdId, HttpError> {
        #[cfg(feature = "plus")]
        Self::rate_limit(context, project_id).await?;
        Self::from_model_inner(
            write_conn!(context),
            project_id,
            branch_id,
            testbed_id,
            measure_id,
            model,
        )
    }

    fn from_model_inner(
        conn: &mut DbConnection,
        project_id: ProjectId,
        branch_id: BranchId,
        testbed_id: TestbedId,
        measure_id: MeasureId,
        model: Model,
    ) -> Result<ThresholdId, HttpError> {
        // Create the new threshold
        let insert_threshold = InsertThreshold::new(project_id, branch_id, testbed_id, measure_id);
        diesel::insert_into(schema::threshold::table)
            .values(&insert_threshold)
            .execute(conn)
            .map_err(resource_conflict_err!(Threshold, insert_threshold))?;

        // Get the new threshold ID
        let threshold_id = QueryThreshold::get_id(conn, insert_threshold.uuid)?;

        // Create the new model
        let insert_model = InsertModel::new(threshold_id, model);
        diesel::insert_into(schema::model::table)
            .values(&insert_model)
            .execute(conn)
            .map_err(resource_conflict_err!(Model, insert_model))?;

        // Get the new model ID
        let model_id = QueryModel::get_id(conn, insert_model.uuid)?;

        // Set the new model for the new threshold
        diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(threshold_id)))
            .set(schema::threshold::model_id.eq(model_id))
            .execute(conn)
            .map_err(resource_conflict_err!(
                Threshold,
                (threshold_id, &insert_model)
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
        Self::from_model_inner(
            conn,
            project_id,
            branch_id,
            testbed_id,
            measure_id,
            Model::lower_boundary(),
        )
    }

    pub fn upper_boundary(
        conn: &mut DbConnection,
        project_id: ProjectId,
        branch_id: BranchId,
        testbed_id: TestbedId,
        measure_id: MeasureId,
    ) -> Result<ThresholdId, HttpError> {
        Self::from_model_inner(
            conn,
            project_id,
            branch_id,
            testbed_id,
            measure_id,
            Model::upper_boundary(),
        )
    }

    pub async fn from_start_point(
        log: &Logger,
        context: &ApiContext,
        query_branch: &QueryBranch,
        branch_start_point: &StartPoint,
    ) -> Result<(), HttpError> {
        let Some(true) = branch_start_point.clone_thresholds else {
            slog::debug!(
                log,
                "Skipping cloning thresholds for start point: {branch_start_point:?}"
            );
            return Ok(());
        };

        assert_siblings(
            BencherResource::Project,
            BencherResource::Branch,
            query_branch.project_id,
            BencherResource::Branch,
            branch_start_point.branch.project_id,
        );

        let mut current_thresholds = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(query_branch.id))
            .load::<QueryThreshold>(auth_conn!(context))
            .map_err(resource_not_found_err!(
                Threshold,
                &branch_start_point.branch
            ))?
            .into_iter()
            .map(|threshold| ((threshold.testbed_id, threshold.measure_id), threshold))
            .collect::<HashMap<_, _>>();
        slog::debug!(log, "Current thresholds: {current_thresholds:?}");

        let start_point_thresholds = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(branch_start_point.branch.id))
            .load::<QueryThreshold>(auth_conn!(context))
            .map_err(resource_not_found_err!(
                Threshold,
                &branch_start_point.branch
            ))?
            .into_iter()
            .map(|threshold| ((threshold.testbed_id, threshold.measure_id), threshold))
            .collect::<HashMap<_, _>>();
        slog::debug!(log, "Start point thresholds: {start_point_thresholds:?}");

        for ((start_point_testbed_id, start_point_measure_id), start_point_threshold) in
            start_point_thresholds
        {
            let start_point_model = start_point_threshold
                .model(auth_conn!(context))?
                .map(QueryModel::into_model);
            slog::debug!(
                log,
                "Processing start point threshold ({start_point_threshold:?}) with model ({start_point_model:?}) for testbed ({start_point_testbed_id}) and measure ({start_point_measure_id})"
            );
            if let Some(current_threshold) =
                current_thresholds.remove(&(start_point_testbed_id, start_point_measure_id))
            {
                slog::debug!(
                    log,
                    "Updating current threshold ({current_threshold:?}) for testbed ({start_point_testbed_id}) and measure ({start_point_measure_id})"
                );
                current_threshold
                    .update_model_if_changed(context, start_point_model)
                    .await?;
                slog::debug!(
                    log,
                    "Updated current threshold ({current_threshold:?}) for testbed ({start_point_testbed_id}) and measure ({start_point_measure_id})"
                );
            } else if let Some(start_point_model) = start_point_model {
                slog::debug!(
                    log,
                    "Creating new threshold from start point ({start_point_model:?}) for testbed ({start_point_testbed_id}) and measure ({start_point_measure_id})"
                );
                Self::from_model(
                    context,
                    query_branch.project_id,
                    query_branch.id,
                    start_point_testbed_id,
                    start_point_measure_id,
                    start_point_model,
                )
                .await?;
                slog::debug!(
                    log,
                    "Created new threshold from start point ({start_point_model:?}) for testbed ({start_point_testbed_id}) and measure ({start_point_measure_id})"
                );
            } else {
                slog::debug!(
                    log,
                    "No model for start point threshold for testbed ({start_point_testbed_id}) and measure ({start_point_measure_id})"
                );
            }
        }

        slog::debug!(log, "Remaining current thresholds: {current_thresholds:?}");
        for (_, current_threshold) in current_thresholds {
            current_threshold.remove_current_model(write_conn!(context))?;
            slog::debug!(
                log,
                "Removed model from current threshold {current_threshold:?}",
            );
        }

        Ok(())
    }

    pub async fn from_report_json(
        log: &Logger,
        context: &ApiContext,
        project_id: ProjectId,
        branch_id: BranchId,
        testbed_id: TestbedId,
        json_thresholds: Option<JsonReportThresholds>,
    ) -> Result<(), HttpError> {
        #[cfg(feature = "plus")]
        Self::rate_limit(context, project_id).await?;

        let Some(json_thresholds) = json_thresholds else {
            slog::debug!(log, "No thresholds in report");
            return Ok(());
        };
        let no_models = json_thresholds
            .models
            .as_ref()
            .is_none_or(HashMap::is_empty);
        let reset_thresholds = json_thresholds.reset.unwrap_or_default();
        if no_models && !reset_thresholds {
            slog::debug!(log, "No threshold models or reset in report");
            return Ok(());
        }

        // Get all thresholds for the report branch and testbed
        let mut current_thresholds = schema::threshold::table
            .filter(schema::threshold::project_id.eq(project_id))
            .filter(schema::threshold::branch_id.eq(branch_id))
            .filter(schema::threshold::testbed_id.eq(testbed_id))
            .load::<QueryThreshold>(auth_conn!(context))
            .map_err(resource_not_found_err!(Threshold, (branch_id, testbed_id)))?
            .into_iter()
            .map(|threshold| (threshold.measure_id, threshold))
            .collect::<HashMap<_, _>>();
        slog::debug!(log, "Current thresholds: {current_thresholds:?}");

        // Iterate over the threshold models in the report.
        // If the threshold does not exist, create it.
        // If it does exist and has changed, update it.
        if let Some(models) = json_thresholds.models {
            for (measure, model) in models {
                let measure_id = QueryMeasure::get_or_create(context, project_id, &measure).await?;
                slog::debug!(log, "Processing threshold for measure {measure_id}");
                if let Some(current_threshold) = current_thresholds.remove(&measure_id) {
                    slog::debug!(log, "Updating threshold for measure {measure_id}");
                    current_threshold
                        .update_model_if_changed(context, Some(model))
                        .await?;
                    slog::debug!(log, "Updated threshold for measure {measure_id}");
                } else {
                    slog::debug!(log, "Creating threshold for measure {measure_id}");
                    Self::from_model(
                        context, project_id, branch_id, testbed_id, measure_id, model,
                    )
                    .await?;
                    slog::debug!(log, "Created threshold for measure {measure_id}");
                }
            }
        }

        slog::debug!(log, "Remaining thresholds: {current_thresholds:?}");
        // If the reset flag is set, remove any thresholds that were not in the report
        if reset_thresholds {
            for (_, current_threshold) in current_thresholds {
                current_threshold.remove_current_model(write_conn!(context))?;
                slog::debug!(log, "Removed model from threshold {current_threshold:?}");
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = threshold_table)]
pub struct UpdateThreshold {
    pub model_id: Option<Option<ModelId>>,
    pub modified: DateTime,
}

impl UpdateThreshold {
    pub fn new_model(conn: &mut DbConnection, model_uuid: ModelUuid) -> Result<Self, HttpError> {
        Ok(Self {
            model_id: Some(Some(QueryModel::get_id(conn, model_uuid)?)),
            modified: DateTime::now(),
        })
    }

    pub fn remove_model() -> Self {
        Self {
            model_id: Some(None),
            modified: DateTime::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use diesel::{
        ExpressionMethods as _, JoinOnDsl as _, NullableExpressionMethods as _, QueryDsl as _,
        RunQueryDsl as _,
    };

    use crate::{
        schema,
        test_util::{
            create_base_entities, create_branch_with_head, create_measure, create_model,
            create_testbed, create_threshold, get_threshold_model_id, get_thresholds_for_branch,
            setup_test_db,
        },
    };

    use super::{QueryThreshold, UpdateThreshold};

    /// Test that thresholds can be queried by `branch_id`.
    /// This is the foundation of threshold cloning.
    #[test]
    fn query_thresholds_by_branch() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        let threshold_id = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000040",
        );

        let thresholds = get_thresholds_for_branch(&mut conn, branch.branch_id);
        assert_eq!(thresholds.len(), 1);
        assert_eq!(thresholds.first(), Some(&threshold_id));
    }

    /// Test threshold model relationship.
    /// Thresholds can optionally have a `model_id` pointing to a model.
    #[test]
    fn threshold_with_model() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        let threshold_id = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000040",
        );

        // Initially no model
        let model_id = get_threshold_model_id(&mut conn, threshold_id);
        assert!(model_id.is_none());

        // Add a model
        let model_id = create_model(
            &mut conn,
            threshold_id,
            "00000000-0000-0000-0000-000000000050",
            0, // test type
        );

        // Now threshold has a model
        let fetched_model_id = get_threshold_model_id(&mut conn, threshold_id);
        assert_eq!(fetched_model_id, Some(model_id));
    }

    /// Test threshold without model.
    /// Thresholds can exist without models.
    #[test]
    fn threshold_without_model() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        let threshold_id = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000040",
        );

        let model_id = get_threshold_model_id(&mut conn, threshold_id);
        assert!(model_id.is_none());
    }

    /// Test collecting thresholds into a `HashMap` by (`testbed_id`, `measure_id`).
    /// This is how `from_start_point` organizes thresholds for matching.
    #[test]
    fn threshold_hashmap_by_testbed_measure() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );

        let testbed1 = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let testbed2 = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000021",
            "ci-runner",
            "ci-runner",
        );

        let measure1 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        let measure2 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000031",
            "throughput",
            "throughput",
        );

        // Create thresholds for different testbed/measure combinations
        let t1 = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed1,
            measure1,
            "00000000-0000-0000-0000-000000000040",
        );
        let t2 = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed1,
            measure2,
            "00000000-0000-0000-0000-000000000041",
        );
        let t3 = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed2,
            measure1,
            "00000000-0000-0000-0000-000000000042",
        );

        // Query and collect into HashMap like from_start_point does
        let thresholds: HashMap<(i32, i32), i32> = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(branch.branch_id))
            .select((
                schema::threshold::testbed_id,
                schema::threshold::measure_id,
                schema::threshold::id,
            ))
            .load::<(i32, i32, i32)>(&mut conn)
            .expect("Failed to query")
            .into_iter()
            .map(|(testbed_id, measure_id, id)| ((testbed_id, measure_id), id))
            .collect();

        assert_eq!(thresholds.len(), 3);
        assert_eq!(thresholds.get(&(testbed1, measure1)), Some(&t1));
        assert_eq!(thresholds.get(&(testbed1, measure2)), Some(&t2));
        assert_eq!(thresholds.get(&(testbed2, measure1)), Some(&t3));
    }

    /// Test matching thresholds between branches by (`testbed_id`, `measure_id`).
    /// This simulates the core matching logic in `from_start_point`.
    #[test]
    #[expect(clippy::too_many_lines, reason = "Test setup requires many entities")]
    fn threshold_matching_between_branches() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        // Source branch
        let source = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );

        // Destination branch
        let dest = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000012",
            "feature",
            "feature",
            "00000000-0000-0000-0000-000000000013",
        );

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure1 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        let measure2 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000031",
            "throughput",
            "throughput",
        );

        // Source has thresholds for measure1 and measure2
        create_threshold(
            &mut conn,
            base.project_id,
            source.branch_id,
            testbed,
            measure1,
            "00000000-0000-0000-0000-000000000040",
        );
        create_threshold(
            &mut conn,
            base.project_id,
            source.branch_id,
            testbed,
            measure2,
            "00000000-0000-0000-0000-000000000041",
        );

        // Dest only has threshold for measure1
        create_threshold(
            &mut conn,
            base.project_id,
            dest.branch_id,
            testbed,
            measure1,
            "00000000-0000-0000-0000-000000000050",
        );

        // Collect source thresholds
        let source_thresholds: HashMap<(i32, i32), i32> = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(source.branch_id))
            .select((
                schema::threshold::testbed_id,
                schema::threshold::measure_id,
                schema::threshold::id,
            ))
            .load::<(i32, i32, i32)>(&mut conn)
            .expect("Failed to query")
            .into_iter()
            .map(|(testbed_id, measure_id, id)| ((testbed_id, measure_id), id))
            .collect();

        // Collect dest thresholds
        let mut dest_thresholds: HashMap<(i32, i32), i32> = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(dest.branch_id))
            .select((
                schema::threshold::testbed_id,
                schema::threshold::measure_id,
                schema::threshold::id,
            ))
            .load::<(i32, i32, i32)>(&mut conn)
            .expect("Failed to query")
            .into_iter()
            .map(|(testbed_id, measure_id, id)| ((testbed_id, measure_id), id))
            .collect();

        assert_eq!(source_thresholds.len(), 2);
        assert_eq!(dest_thresholds.len(), 1);

        // Simulate from_start_point matching logic
        let mut matched = Vec::new();
        let mut new_thresholds_needed = Vec::new();

        for (testbed_id, measure_id) in source_thresholds.keys() {
            if let Some(dest_threshold_id) = dest_thresholds.remove(&(*testbed_id, *measure_id)) {
                matched.push(dest_threshold_id);
            } else {
                new_thresholds_needed.push((*testbed_id, *measure_id));
            }
        }

        // One matched (measure1), one new needed (measure2)
        assert_eq!(matched.len(), 1);
        assert_eq!(new_thresholds_needed.len(), 1);
        assert_eq!(new_thresholds_needed.first(), Some(&(testbed, measure2)));

        // No orphans in dest (dest_thresholds is now empty after remove)
        assert!(dest_thresholds.is_empty());
    }

    /// Test orphan threshold detection.
    /// Dest thresholds not in source should be identified as orphans.
    #[test]
    fn orphan_threshold_detection() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let source = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );

        let dest = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000012",
            "feature",
            "feature",
            "00000000-0000-0000-0000-000000000013",
        );

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure1 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        let measure2 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000031",
            "throughput",
            "throughput",
        );

        // Source has only measure1
        create_threshold(
            &mut conn,
            base.project_id,
            source.branch_id,
            testbed,
            measure1,
            "00000000-0000-0000-0000-000000000040",
        );

        // Dest has both measure1 and measure2
        create_threshold(
            &mut conn,
            base.project_id,
            dest.branch_id,
            testbed,
            measure1,
            "00000000-0000-0000-0000-000000000050",
        );
        let orphan_threshold_id = create_threshold(
            &mut conn,
            base.project_id,
            dest.branch_id,
            testbed,
            measure2,
            "00000000-0000-0000-0000-000000000051",
        );

        // Collect source thresholds
        let source_thresholds: HashMap<(i32, i32), i32> = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(source.branch_id))
            .select((
                schema::threshold::testbed_id,
                schema::threshold::measure_id,
                schema::threshold::id,
            ))
            .load::<(i32, i32, i32)>(&mut conn)
            .expect("Failed to query")
            .into_iter()
            .map(|(testbed_id, measure_id, id)| ((testbed_id, measure_id), id))
            .collect();

        // Collect dest thresholds
        let mut dest_thresholds: HashMap<(i32, i32), i32> = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(dest.branch_id))
            .select((
                schema::threshold::testbed_id,
                schema::threshold::measure_id,
                schema::threshold::id,
            ))
            .load::<(i32, i32, i32)>(&mut conn)
            .expect("Failed to query")
            .into_iter()
            .map(|(testbed_id, measure_id, id)| ((testbed_id, measure_id), id))
            .collect();

        // Process source thresholds (removes matching from dest)
        for (testbed_id, measure_id) in source_thresholds.keys() {
            dest_thresholds.remove(&(*testbed_id, *measure_id));
        }

        // Remaining dest_thresholds are orphans
        assert_eq!(dest_thresholds.len(), 1);
        assert!(
            dest_thresholds
                .values()
                .any(|&id| id == orphan_threshold_id)
        );
    }

    /// Test that `InsertThreshold` creates valid records.
    #[test]
    fn insert_threshold() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        // Use create_threshold helper which does the insertion
        let threshold_id = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000040",
        );

        let threshold: QueryThreshold = schema::threshold::table
            .filter(schema::threshold::id.eq(threshold_id))
            .first(&mut conn)
            .expect("Failed to query threshold");

        // Use i32::from() to extract values from typed IDs
        assert_eq!(i32::from(threshold.project_id), base.project_id);
        assert_eq!(i32::from(threshold.branch_id), branch.branch_id);
        assert_eq!(i32::from(threshold.testbed_id), testbed);
        assert_eq!(i32::from(threshold.measure_id), measure);
        assert!(threshold.model_id.is_none());
    }

    /// Test removing model from threshold.
    /// The `UpdateThreshold::remove_model()` sets `model_id` to `None`.
    #[test]
    fn remove_model_from_threshold() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        let threshold_id = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000040",
        );

        // Add a model
        create_model(
            &mut conn,
            threshold_id,
            "00000000-0000-0000-0000-000000000050",
            0,
        );

        // Verify model exists
        assert!(get_threshold_model_id(&mut conn, threshold_id).is_some());

        // Remove model using UpdateThreshold
        let update_threshold = UpdateThreshold::remove_model();
        diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(threshold_id)))
            .set(&update_threshold)
            .execute(&mut conn)
            .expect("Failed to update threshold");

        // Verify model is removed
        assert!(get_threshold_model_id(&mut conn, threshold_id).is_none());
    }

    /// Test threshold model relationship via JOIN query.
    /// This tests the LEFT JOIN pattern used in the optimized `from_start_point`.
    #[test]
    fn threshold_model_join_query() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure1 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        let measure2 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000031",
            "throughput",
            "throughput",
        );

        // Threshold 1 with model
        let t1 = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure1,
            "00000000-0000-0000-0000-000000000040",
        );
        create_model(&mut conn, t1, "00000000-0000-0000-0000-000000000050", 0);

        // Threshold 2 without model
        let t2 = create_threshold(
            &mut conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure2,
            "00000000-0000-0000-0000-000000000041",
        );

        // Use LEFT JOIN to get thresholds with optional models

        let results: Vec<(i32, Option<i32>)> = schema::threshold::table
            .left_join(
                schema::model::table
                    .on(schema::model::id.nullable().eq(schema::threshold::model_id)),
            )
            .filter(schema::threshold::branch_id.eq(branch.branch_id))
            .select((schema::threshold::id, schema::model::id.nullable()))
            .load(&mut conn)
            .expect("Failed to query");

        assert_eq!(results.len(), 2);

        // Find results by threshold id
        let t1_result = results.iter().find(|(id, _)| *id == t1);
        let t2_result = results.iter().find(|(id, _)| *id == t2);

        assert!(t1_result.unwrap().1.is_some()); // t1 has model
        assert!(t2_result.unwrap().1.is_none()); // t2 has no model
    }

    /// Test multiple thresholds with models using JOIN.
    /// Ensures the JOIN pattern works correctly with multiple thresholds.
    #[test]
    fn multiple_thresholds_with_models_join() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        // Create 5 thresholds, 3 with models, 2 without
        let mut thresholds_with_models = Vec::new();
        let mut thresholds_without_models = Vec::new();

        for i in 0..5 {
            let uuid_suffix = 30 + i;
            let measure = create_measure(
                &mut conn,
                base.project_id,
                &format!("00000000-0000-0000-0000-0000000000{uuid_suffix:02}"),
                &format!("measure{i}"),
                &format!("measure{i}"),
            );

            let threshold_suffix = 40 + i;
            let threshold_id = create_threshold(
                &mut conn,
                base.project_id,
                branch.branch_id,
                testbed,
                measure,
                &format!("00000000-0000-0000-0000-0000000000{threshold_suffix:02}"),
            );

            if i < 3 {
                let model_suffix = 50 + i;
                create_model(
                    &mut conn,
                    threshold_id,
                    &format!("00000000-0000-0000-0000-0000000000{model_suffix:02}"),
                    0,
                );
                thresholds_with_models.push(threshold_id);
            } else {
                thresholds_without_models.push(threshold_id);
            }
        }

        // Use LEFT JOIN to fetch all at once

        let results: Vec<(i32, Option<i32>)> = schema::threshold::table
            .left_join(
                schema::model::table
                    .on(schema::model::id.nullable().eq(schema::threshold::model_id)),
            )
            .filter(schema::threshold::branch_id.eq(branch.branch_id))
            .select((schema::threshold::id, schema::model::id.nullable()))
            .load(&mut conn)
            .expect("Failed to query");

        assert_eq!(results.len(), 5);

        let with_model_count = results.iter().filter(|(_, m)| m.is_some()).count();
        let without_model_count = results.iter().filter(|(_, m)| m.is_none()).count();

        assert_eq!(with_model_count, 3);
        assert_eq!(without_model_count, 2);

        // Verify specific thresholds
        for t in &thresholds_with_models {
            let result = results.iter().find(|(id, _)| id == t).unwrap();
            assert!(result.1.is_some());
        }

        for t in &thresholds_without_models {
            let result = results.iter().find(|(id, _)| id == t).unwrap();
            assert!(result.1.is_none());
        }
    }

    /// Test the complete `from_start_point` matching algorithm simulation.
    /// This simulates all cases: update existing, create new, remove orphan.
    #[test]
    #[expect(
        clippy::too_many_lines,
        reason = "Comprehensive test requires many entities"
    )]
    fn from_start_point_matching_simulation() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        // Source branch (start point)
        let source = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );

        // Current branch
        let current = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000012",
            "feature",
            "feature",
            "00000000-0000-0000-0000-000000000013",
        );

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure_shared = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "shared",
            "shared",
        );

        let measure_source_only = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000031",
            "source_only",
            "source-only",
        );

        let measure_current_only = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000032",
            "current_only",
            "current-only",
        );

        // Source thresholds: shared (with model), source_only (with model)
        let source_shared = create_threshold(
            &mut conn,
            base.project_id,
            source.branch_id,
            testbed,
            measure_shared,
            "00000000-0000-0000-0000-000000000040",
        );
        create_model(
            &mut conn,
            source_shared,
            "00000000-0000-0000-0000-000000000060",
            0,
        );

        let source_only = create_threshold(
            &mut conn,
            base.project_id,
            source.branch_id,
            testbed,
            measure_source_only,
            "00000000-0000-0000-0000-000000000041",
        );
        create_model(
            &mut conn,
            source_only,
            "00000000-0000-0000-0000-000000000061",
            0,
        );

        // Current thresholds: shared (with model), current_only (with model)
        let current_shared = create_threshold(
            &mut conn,
            base.project_id,
            current.branch_id,
            testbed,
            measure_shared,
            "00000000-0000-0000-0000-000000000050",
        );
        create_model(
            &mut conn,
            current_shared,
            "00000000-0000-0000-0000-000000000070",
            0,
        );

        let current_only = create_threshold(
            &mut conn,
            base.project_id,
            current.branch_id,
            testbed,
            measure_current_only,
            "00000000-0000-0000-0000-000000000051",
        );
        create_model(
            &mut conn,
            current_only,
            "00000000-0000-0000-0000-000000000071",
            0,
        );

        // Simulate from_start_point logic
        let source_thresholds: HashMap<(i32, i32), (i32, Option<i32>)> = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(source.branch_id))
            .select((
                schema::threshold::testbed_id,
                schema::threshold::measure_id,
                schema::threshold::id,
                schema::threshold::model_id,
            ))
            .load::<(i32, i32, i32, Option<i32>)>(&mut conn)
            .expect("Failed to query")
            .into_iter()
            .map(|(t, m, id, model)| ((t, m), (id, model)))
            .collect();

        let mut current_thresholds: HashMap<(i32, i32), i32> = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(current.branch_id))
            .select((
                schema::threshold::testbed_id,
                schema::threshold::measure_id,
                schema::threshold::id,
            ))
            .load::<(i32, i32, i32)>(&mut conn)
            .expect("Failed to query")
            .into_iter()
            .map(|(t, m, id)| ((t, m), id))
            .collect();

        assert_eq!(source_thresholds.len(), 2);
        assert_eq!(current_thresholds.len(), 2);

        let mut to_update = Vec::new();
        let mut to_create = Vec::new();

        for ((testbed_id, measure_id), (_source_id, source_model)) in &source_thresholds {
            if let Some(current_id) = current_thresholds.remove(&(*testbed_id, *measure_id)) {
                // Match found - would update
                to_update.push((current_id, *source_model));
            } else if source_model.is_some() {
                // No match but source has model - would create
                to_create.push((*testbed_id, *measure_id));
            }
        }

        // Remaining current_thresholds are orphans
        let orphans: Vec<_> = current_thresholds.values().copied().collect();

        // Verify expected behavior
        assert_eq!(to_update.len(), 1); // shared
        assert_eq!(to_create.len(), 1); // source_only
        assert_eq!(orphans.len(), 1); // current_only

        // Verify specific thresholds
        assert_eq!(to_update.first().map(|(id, _)| *id), Some(current_shared));
        assert_eq!(to_create.first(), Some(&(testbed, measure_source_only)));
        assert_eq!(orphans.first(), Some(&current_only));
    }

    /// Test that thresholds from different branches are isolated.
    /// Queries should only return thresholds for the specified branch.
    #[test]
    fn threshold_branch_isolation() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let branch1 = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "branch1",
            "branch1",
            "00000000-0000-0000-0000-000000000011",
        );

        let branch2 = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000012",
            "branch2",
            "branch2",
            "00000000-0000-0000-0000-000000000013",
        );

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        let measure = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );

        // Create threshold for branch1
        let t1 = create_threshold(
            &mut conn,
            base.project_id,
            branch1.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000040",
        );

        // Create threshold for branch2
        let t2 = create_threshold(
            &mut conn,
            base.project_id,
            branch2.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000041",
        );

        // Query branch1 thresholds
        let branch1_thresholds = get_thresholds_for_branch(&mut conn, branch1.branch_id);
        assert_eq!(branch1_thresholds.len(), 1);
        assert_eq!(branch1_thresholds.first(), Some(&t1));

        // Query branch2 thresholds
        let branch2_thresholds = get_thresholds_for_branch(&mut conn, branch2.branch_id);
        assert_eq!(branch2_thresholds.len(), 1);
        assert_eq!(branch2_thresholds.first(), Some(&t2));
    }

    /// Test large threshold set with models.
    /// Ensures the system can handle many thresholds efficiently.
    #[test]
    fn large_threshold_set() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);

        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );

        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );

        // Create 20 thresholds with models
        for i in 0..20 {
            let uuid_suffix = 30 + i;
            let measure = create_measure(
                &mut conn,
                base.project_id,
                &format!("00000000-0000-0000-0000-0000000000{uuid_suffix:02}"),
                &format!("measure{i}"),
                &format!("measure{i}"),
            );

            let threshold_suffix = 40 + i;
            let threshold_id = create_threshold(
                &mut conn,
                base.project_id,
                branch.branch_id,
                testbed,
                measure,
                &format!("00000000-0000-0000-0000-0000000000{threshold_suffix:02}"),
            );

            let model_suffix = 60 + i;
            create_model(
                &mut conn,
                threshold_id,
                &format!("00000000-0000-0000-0000-0000000000{model_suffix:02}"),
                0,
            );
        }

        let thresholds = get_thresholds_for_branch(&mut conn, branch.branch_id);
        assert_eq!(thresholds.len(), 20);

        // Verify all have models using JOIN query

        let results: Vec<(i32, Option<i32>)> = schema::threshold::table
            .left_join(
                schema::model::table
                    .on(schema::model::id.nullable().eq(schema::threshold::model_id)),
            )
            .filter(schema::threshold::branch_id.eq(branch.branch_id))
            .select((schema::threshold::id, schema::model::id.nullable()))
            .load(&mut conn)
            .expect("Failed to query");

        assert_eq!(results.len(), 20);
        assert!(results.iter().all(|(_, m)| m.is_some()));
    }
}
