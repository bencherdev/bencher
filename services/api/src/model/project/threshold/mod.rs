use bencher_json::{
    project::threshold::{JsonThreshold, JsonThresholdModel},
    DateTime, Model, ModelUuid, ThresholdUuid,
};
use diesel::{BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use self::model::{InsertModel, ModelId, QueryModel};
use super::{
    branch::{reference::ReferenceId, version::VersionId, BranchId, QueryBranch},
    measure::{MeasureId, QueryMeasure},
    testbed::{QueryTestbed, TestbedId},
    ProjectId, QueryProject,
};
use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::{assert_parentage, resource_conflict_err, resource_not_found_err, BencherResource},
    schema::{self, threshold as threshold_table},
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

    pub fn model(&self, conn: &mut DbConnection) -> Result<Option<QueryModel>, HttpError> {
        if let Some(model_id) = self.model_id {
            Ok(Some(QueryModel::get(conn, model_id)?))
        } else {
            Ok(None)
        }
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

        // Update the old model to be replaced, if there is one
        if let Some(model_id) = self.model_id {
            diesel::update(schema::model::table.filter(schema::model::id.eq(model_id)))
                .set(schema::model::replaced.eq(Some(DateTime::now())))
                .execute(conn)
                .map_err(resource_conflict_err!(Model, model_id))?;
        }

        Ok(())
    }

    pub async fn get_alert_json(
        context: &ApiContext,
        threshold_id: ThresholdId,
        model_id: ModelId,
        reference_id: ReferenceId,
        version_id: VersionId,
    ) -> Result<JsonThreshold, HttpError> {
        let query_threshold = Self::get(conn_lock!(context), threshold_id)?;
        let query_model = QueryModel::get(conn_lock!(context), model_id)?;
        query_threshold
            .into_json_for_model(context, Some(query_model), Some((reference_id, version_id)))
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
        reference_version: Option<(ReferenceId, VersionId)>,
    ) -> Result<JsonThreshold, HttpError> {
        let model = if let Some(query_model) = &query_model {
            assert_parentage(
                BencherResource::Threshold,
                self.id,
                BencherResource::Model,
                query_model.threshold_id,
            );
            Some(query_model.into_json_for_threshold(&self))
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
        let branch = if let Some((reference_id, version_id)) = reference_version {
            QueryBranch::get_json_for_report(context, &query_project, reference_id, version_id)
                .await?
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
        let model = query_model.into_json_for_threshold(&self);
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
