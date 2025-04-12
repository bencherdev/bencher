use std::collections::HashMap;

use bencher_json::{
    project::{
        report::JsonReportThresholds,
        threshold::{JsonThreshold, JsonThresholdModel},
    },
    DateTime, Model, ModelUuid, ThresholdUuid,
};
use diesel::{BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;
use model::UpdateModel;
use slog::Logger;

use self::model::{InsertModel, ModelId, QueryModel};
use super::{
    branch::{head::HeadId, start_point::StartPoint, version::VersionId, BranchId, QueryBranch},
    measure::{MeasureId, QueryMeasure},
    testbed::{QueryTestbed, TestbedId},
    ProjectId, QueryProject,
};
use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::{
        assert_parentage, assert_siblings, resource_conflict_err, resource_not_found_err,
        BencherResource,
    },
    macros::fn_get::{fn_get, fn_get_id, fn_get_uuid},
    schema::{self, threshold as threshold_table},
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
            (Some(_), None) => self.remove_current_model(conn_lock!(context)),
            // Current model and new model,
            // update the current if it has changed.
            (Some(model_id), Some(model)) => {
                let current_model = QueryModel::get(conn_lock!(context), model_id)?.into_model();
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
        self.update_from_model_inner(conn_lock!(context), model)
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

    pub async fn get_alert_json(
        context: &ApiContext,
        threshold_id: ThresholdId,
        model_id: ModelId,
        head_id: HeadId,
        version_id: VersionId,
    ) -> Result<JsonThreshold, HttpError> {
        let query_threshold = Self::get(conn_lock!(context), threshold_id)?;
        let query_model = QueryModel::get(conn_lock!(context), model_id)?;
        query_threshold
            .into_json_for_model(context, Some(query_model), Some((head_id, version_id)))
            .await
    }

    pub async fn into_json(self, context: &ApiContext) -> Result<JsonThreshold, HttpError> {
        let query_model = self.model(conn_lock!(context))?;
        self.into_json_for_model(context, query_model, None).await
    }

    pub async fn into_json_for_model(
        self,
        context: &ApiContext,
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
        let query_project = QueryProject::get(conn_lock!(context), project_id)?;
        let branch = if let Some((head_id, version_id)) = head_version {
            QueryBranch::get_json_for_report(context, &query_project, head_id, version_id).await?
        } else {
            let query_branch = QueryBranch::get(conn_lock!(context), branch_id)?;
            query_branch.into_json_for_project(conn_lock!(context), &query_project)?
        };
        let testbed = QueryTestbed::get(conn_lock!(context), testbed_id)?
            .into_json_for_project(&query_project);
        let measure = QueryMeasure::get(conn_lock!(context), measure_id)?
            .into_json_for_project(&query_project);
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
            conn_lock!(context),
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
            .load::<QueryThreshold>(conn_lock!(context))
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
            .load::<QueryThreshold>(conn_lock!(context))
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
                .model(conn_lock!(context))?
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
            current_threshold.remove_current_model(conn_lock!(context))?;
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
            .map_or(true, HashMap::is_empty);
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
            .load::<QueryThreshold>(conn_lock!(context))
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
                current_threshold.remove_current_model(conn_lock!(context))?;
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
