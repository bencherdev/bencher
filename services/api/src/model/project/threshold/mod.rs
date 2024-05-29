use bencher_json::{
    project::threshold::{JsonThreshold, JsonThresholdModel},
    DateTime, Model, ModelUuid, ThresholdUuid,
};
use diesel::{BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;
use http::StatusCode;

use self::model::{InsertModel, ModelId, QueryModel};
use super::{
    branch::{BranchId, QueryBranch},
    measure::{MeasureId, QueryMeasure},
    testbed::{QueryTestbed, TestbedId},
    ProjectId, QueryProject,
};
use crate::{
    context::DbConnection,
    error::{
        assert_parentage, issue_error, resource_conflict_err, resource_not_found_err,
        BencherResource,
    },
    schema::threshold as threshold_table,
    schema::{self},
    util::fn_get::{fn_get, fn_get_id, fn_get_uuid},
};

pub mod alert;
pub mod boundary;
pub mod model;

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

    pub fn model_id(&self) -> Result<ModelId, HttpError> {
        self.model_id.ok_or_else(|| {
            // A threshold should always have a model
            let err = issue_error(
                StatusCode::NOT_FOUND,
                "Failed to find threshold model",
                &format!("No threshold model: {}/{}", self.project_id, self.uuid),
                "threshold model is null",
            );
            debug_assert!(false, "{err}");
            #[cfg(feature = "sentry")]
            sentry::capture_error(&err);
            err
        })
    }

    pub fn get_with_model(
        conn: &mut DbConnection,
        threshold_id: ThresholdId,
        model_id: ModelId,
    ) -> Result<Self, HttpError> {
        let mut threshold = Self::get(conn, threshold_id)?;
        // IMPORTANT: Set the model ID to the one specified and not the current value!
        threshold.model_id = Some(model_id);
        Ok(threshold)
    }

    pub fn get_json(
        conn: &mut DbConnection,
        threshold_id: ThresholdId,
        model_id: ModelId,
    ) -> Result<JsonThreshold, HttpError> {
        Self::get_with_model(conn, threshold_id, model_id)?.into_json(conn)
    }

    pub fn update_from_json(&self, conn: &mut DbConnection, model: Model) -> Result<(), HttpError> {
        // Insert the new model
        let insert_model = InsertModel::from_json(self.id, model);
        diesel::insert_into(schema::model::table)
            .values(&insert_model)
            .execute(conn)
            .map_err(resource_conflict_err!(Model, (self, &insert_model)))?;

        // Update the current threshold to use the new model
        let update_threshold = UpdateThreshold::new_model(conn, insert_model.uuid)?;
        diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(self.id)))
            .set(&update_threshold)
            .execute(conn)
            .map_err(resource_conflict_err!(Threshold, (&self, &insert_model)))?;

        // Update the old model to be replaced
        let model_id = self.model_id()?;
        diesel::update(schema::model::table.filter(schema::model::id.eq(model_id)))
            .set(schema::model::replaced.eq(Some(DateTime::now())))
            .execute(conn)
            .map_err(resource_conflict_err!(Model, model_id))?;

        Ok(())
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonThreshold, HttpError> {
        let model_id = self.model_id()?;
        let query_model = QueryModel::get(conn, model_id)?;
        self.into_json_for_model(conn, query_model)
    }

    pub fn into_json_for_model(
        self,
        conn: &mut DbConnection,
        query_model: QueryModel,
    ) -> Result<JsonThreshold, HttpError> {
        assert_parentage(
            BencherResource::Threshold,
            self.id,
            BencherResource::Model,
            query_model.threshold_id,
        );
        let model = query_model.into_json_for_threshold(&self);
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
        Ok(JsonThreshold {
            uuid,
            project: QueryProject::get_uuid(conn, project_id)?,
            branch: QueryBranch::get(conn, branch_id)?.into_json(conn)?,
            testbed: QueryTestbed::get(conn, testbed_id)?.into_json(conn)?,
            measure: QueryMeasure::get(conn, measure_id)?.into_json(conn)?,
            // TODO remove in due time
            statistic: Some(model),
            model,
            created,
            modified,
        })
    }

    pub fn into_threshold_model_json(
        self,
        conn: &mut DbConnection,
    ) -> Result<JsonThresholdModel, HttpError> {
        let project = QueryProject::get(conn, self.project_id)?;
        let model_id = self.model_id()?;
        let model = QueryModel::get(conn, model_id)?;
        Ok(self.into_threshold_model_json_for_project(&project, model))
    }

    pub fn into_threshold_model_json_for_project(
        self,
        project: &QueryProject,
        model: QueryModel,
    ) -> JsonThresholdModel {
        let model = model.into_json_for_threshold(&self);
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
            // TODO remove in due time
            statistic: Some(model),
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

    pub fn insert_from_json(
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
        let insert_model = InsertModel::from_json(threshold_id, model);
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
        Self::insert_from_json(
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
        Self::insert_from_json(
            conn,
            project_id,
            branch_id,
            testbed_id,
            measure_id,
            Model::upper_boundary(),
        )
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = threshold_table)]
pub struct UpdateThreshold {
    pub model_id: ModelId,
    pub modified: DateTime,
}

impl UpdateThreshold {
    pub fn new_model(conn: &mut DbConnection, model_uuid: ModelUuid) -> Result<Self, HttpError> {
        Ok(Self {
            model_id: QueryModel::get_id(conn, model_uuid)?,
            modified: DateTime::now(),
        })
    }
}
