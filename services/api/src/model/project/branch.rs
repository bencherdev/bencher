use bencher_json::{
    project::branch::{JsonBranchVersion, JsonStartPoint, JsonUpdateBranch, BRANCH_MAIN_STR},
    BranchName, BranchUuid, DateTime, JsonBranch, JsonNewBranch, Slug,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;
use http::StatusCode;

use super::{
    branch_version::InsertBranchVersion,
    threshold::model::{InsertModel, QueryModel},
    version::{QueryVersion, VersionId},
    ProjectId, QueryProject,
};
use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::{
        assert_parentage, issue_error, resource_conflict_err, resource_not_found_err,
        BencherResource,
    },
    model::project::threshold::{InsertThreshold, QueryThreshold},
    schema::{self, branch as branch_table},
    util::{
        fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
        name_id::{fn_eq_name_id, fn_from_name_id},
        resource_id::{fn_eq_resource_id, fn_from_resource_id},
        slug::ok_slug,
    },
};

crate::util::typed_id::typed_id!(BranchId);

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = branch_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
pub struct QueryBranch {
    pub id: BranchId,
    pub uuid: BranchUuid,
    pub project_id: ProjectId,
    pub name: BranchName,
    pub slug: Slug,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryBranch {
    fn_eq_resource_id!(branch);
    fn_from_resource_id!(branch, Branch);

    fn_eq_name_id!(BranchName, branch);
    fn_from_name_id!(branch, Branch);

    fn_get!(branch, BranchId);
    fn_get_id!(branch, BranchId, BranchUuid);
    fn_get_uuid!(branch, BranchId, BranchUuid);
    fn_from_uuid!(branch, BranchUuid, Branch);

    pub fn get_branch_version_json(
        conn: &mut DbConnection,
        branch_id: BranchId,
        version_id: VersionId,
    ) -> Result<JsonBranchVersion, HttpError> {
        let JsonBranch {
            uuid,
            project,
            name,
            slug,
            created,
            modified,
        } = Self::get(conn, branch_id)?.into_json(conn)?;
        Ok(JsonBranchVersion {
            uuid,
            project,
            name,
            slug,
            version: QueryVersion::get(conn, version_id)?.into_json(),
            created,
            modified,
        })
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonBranch, HttpError> {
        let project = QueryProject::get(conn, self.project_id)?;
        Ok(self.into_json_for_project(&project))
    }

    pub fn into_json_for_project(self, project: &QueryProject) -> JsonBranch {
        let Self {
            uuid,
            project_id,
            name,
            slug,
            created,
            modified,
            ..
        } = self;
        assert_parentage(
            BencherResource::Project,
            project.id,
            BencherResource::Branch,
            project_id,
        );
        JsonBranch {
            uuid,
            project: project.uuid,
            name,
            slug,
            created,
            modified,
        }
    }

    pub fn is_system(&self) -> bool {
        matches!(self.name.as_ref(), BRANCH_MAIN_STR)
            || matches!(self.slug.as_ref(), BRANCH_MAIN_STR)
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = branch_table)]
pub struct InsertBranch {
    pub uuid: BranchUuid,
    pub project_id: ProjectId,
    pub name: BranchName,
    pub slug: Slug,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertBranch {
    pub fn from_json(
        conn: &mut DbConnection,
        project_id: ProjectId,
        branch: JsonNewBranch,
    ) -> Result<Self, HttpError> {
        let JsonNewBranch { name, slug, .. } = branch;
        let slug = ok_slug!(conn, project_id, &name, slug, branch, QueryBranch)?;
        let timestamp = DateTime::now();
        Ok(Self {
            uuid: BranchUuid::new(),
            project_id,
            name,
            slug,
            created: timestamp,
            modified: timestamp,
        })
    }

    pub fn main(conn: &mut DbConnection, project_id: ProjectId) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, JsonNewBranch::main())
    }

    pub async fn start_point(
        &self,
        context: &ApiContext,
        start_point: &JsonStartPoint,
    ) -> Result<(), HttpError> {
        let JsonStartPoint { branch, thresholds } = start_point;

        let start_point_branch_id =
            QueryBranch::from_name_id(conn_lock!(context), self.project_id, branch)?.id;
        let new_branch_id = QueryBranch::get_id(conn_lock!(context), self.uuid)?;

        self.clone_versions(context, new_branch_id, start_point_branch_id)
            .await?;

        if let Some(true) = thresholds {
            self.clone_thresholds(context, new_branch_id, start_point_branch_id)
                .await?;
        }

        Ok(())
    }

    async fn clone_versions(
        &self,
        context: &ApiContext,
        new_branch_id: BranchId,
        start_point_branch_id: BranchId,
    ) -> Result<(), HttpError> {
        // Get all versions for the start point branch
        let version_ids = schema::branch_version::table
            .filter(schema::branch_version::branch_id.eq(start_point_branch_id))
            .select(schema::branch_version::version_id)
            .load::<VersionId>(conn_lock!(context))
            .map_err(resource_not_found_err!(
                BranchVersion,
                start_point_branch_id
            ))?;

        // Add new branch to all start point branch versions
        for version_id in version_ids {
            let insert_branch_version = InsertBranchVersion {
                branch_id: new_branch_id,
                version_id,
            };

            diesel::insert_into(schema::branch_version::table)
                .values(&insert_branch_version)
                .execute(conn_lock!(context))
                .map_err(resource_conflict_err!(BranchVersion, insert_branch_version))?;
        }

        Ok(())
    }

    async fn clone_thresholds(
        &self,
        context: &ApiContext,
        new_branch_id: BranchId,
        start_point_branch_id: BranchId,
    ) -> Result<(), HttpError> {
        // Get all thresholds for the start point branch
        let query_thresholds = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(start_point_branch_id))
            .load::<QueryThreshold>(conn_lock!(context))
            .map_err(resource_not_found_err!(Threshold, start_point_branch_id))?;

        // Add new branch to cloned thresholds with cloned current threshold model
        for query_threshold in query_thresholds {
            // Hold the database lock across the entire `clone_threshold` call
            self.clone_threshold(conn_lock!(context), new_branch_id, query_threshold)?;
        }

        Ok(())
    }

    fn clone_threshold(
        &self,
        conn: &mut DbConnection,
        new_branch_id: BranchId,
        query_threshold: QueryThreshold,
    ) -> Result<(), HttpError> {
        // Clone the threshold for the new branch
        let insert_threshold = InsertThreshold::new(
            self.project_id,
            new_branch_id,
            query_threshold.testbed_id,
            query_threshold.measure_id,
        );

        // Create the new threshold
        diesel::insert_into(schema::threshold::table)
            .values(&insert_threshold)
            .execute(conn)
            .map_err(resource_conflict_err!(Threshold, insert_threshold))?;

        // If there is a model, clone that too
        let Some(model_id) = query_threshold.model_id else {
            let err = issue_error(
                StatusCode::NOT_FOUND,
                "Failed to find threshold model",
                &format!(
                    "No threshold model: {}/{}",
                    self.project_id, query_threshold.uuid
                ),
                "threshold model is null",
            );
            debug_assert!(false, "{err}");
            #[cfg(feature = "sentry")]
            sentry::capture_error(&err);
            return Err(err);
        };

        // Get the new threshold
        let threshold_id = QueryThreshold::get_id(conn, insert_threshold.uuid)?;

        // Get the current threshold model
        let query_model = schema::model::table
            .filter(schema::model::id.eq(model_id))
            .first::<QueryModel>(conn)
            .map_err(resource_not_found_err!(Model, query_threshold))?;

        // Clone the current threshold model
        let mut insert_model = InsertModel::from(query_model.clone());
        // Set the cloned model to the new threshold
        insert_model.threshold_id = threshold_id;
        diesel::insert_into(schema::model::table)
            .values(&insert_model)
            .execute(conn)
            .map_err(resource_conflict_err!(Model, insert_model))?;

        // Get the new model
        let model_id = QueryModel::get_id(conn, insert_model.uuid)?;

        // Set the new model for the new threshold
        diesel::update(schema::threshold::table.filter(schema::threshold::id.eq(threshold_id)))
            .set(schema::threshold::model_id.eq(model_id))
            .execute(conn)
            .map_err(resource_conflict_err!(
                Threshold,
                (&query_threshold, &query_model)
            ))?;

        Ok(())
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = branch_table)]
pub struct UpdateBranch {
    pub name: Option<BranchName>,
    pub slug: Option<Slug>,
    pub modified: DateTime,
}

impl From<JsonUpdateBranch> for UpdateBranch {
    fn from(update: JsonUpdateBranch) -> Self {
        let JsonUpdateBranch { name, slug } = update;
        Self {
            name,
            slug,
            modified: DateTime::now(),
        }
    }
}
