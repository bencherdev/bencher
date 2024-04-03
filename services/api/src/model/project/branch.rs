use bencher_json::{
    project::branch::{JsonBranchVersion, JsonNewStartPoint, JsonUpdateBranch, BRANCH_MAIN_STR},
    BranchName, BranchUuid, DateTime, JsonBranch, JsonNewBranch, Slug,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;
use slog::Logger;

use super::{
    branch_version::{BranchVersionId, InsertBranchVersion, QueryBranchVersion},
    threshold::model::{InsertModel, QueryModel},
    version::{QueryVersion, VersionId},
    ProjectId, QueryProject,
};
use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::{assert_parentage, resource_conflict_err, resource_not_found_err, BencherResource},
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
    pub start_point_id: Option<BranchVersionId>,
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
            start_point,
            created,
            modified,
        } = Self::get(conn, branch_id)?.into_json(conn)?;
        Ok(JsonBranchVersion {
            uuid,
            project,
            name,
            slug,
            version: QueryVersion::get(conn, version_id)?.into_json(),
            start_point,
            created,
            modified,
        })
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonBranch, HttpError> {
        let project = QueryProject::get(conn, self.project_id)?;
        self.into_json_for_project(conn, &project)
    }

    pub fn into_json_for_project(
        self,
        conn: &mut DbConnection,
        project: &QueryProject,
    ) -> Result<JsonBranch, HttpError> {
        let Self {
            uuid,
            project_id,
            name,
            slug,
            start_point_id,
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
        let start_point = if let Some(start_point_id) = start_point_id {
            Some(QueryBranchVersion::get(conn, start_point_id)?.into_json(conn)?)
        } else {
            None
        };
        Ok(JsonBranch {
            uuid,
            project: project.uuid,
            name,
            slug,
            start_point,
            created,
            modified,
        })
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
    pub start_point_id: Option<BranchVersionId>,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertBranch {
    pub fn from_json(
        conn: &mut DbConnection,
        project_id: ProjectId,
        branch: JsonNewBranch,
    ) -> Result<Self, HttpError> {
        let JsonNewBranch {
            name,
            slug,
            start_point,
            ..
        } = branch;
        let slug = ok_slug!(conn, project_id, &name, slug, branch, QueryBranch)?;
        let timestamp = DateTime::now();

        let start_point_id = if let Some(JsonNewStartPoint { branch, hash, .. }) = start_point {
            // Get the start point branch
            let start_point_branch = QueryBranch::from_name_id(conn, project_id, &branch)?;
            let mut query = schema::branch_version::table
                .inner_join(schema::version::table)
                // Filter for the start point branch
                .filter(schema::branch_version::branch_id.eq(start_point_branch.id))
                // Sanity check that we are in the right project
                .filter(schema::version::project_id.eq(project_id))
                .into_boxed();

            if let Some(hash) = hash.as_ref() {
                // Make sure the start point version has the correct hash, if specified.
                query = query.filter(schema::version::hash.eq(hash));
            }

            Some(
                query
                    // If the hash is not specified, get the most recent version.
                    .order(schema::version::number.desc())
                    .select(schema::branch_version::id)
                    .first::<BranchVersionId>(conn)
                    .map_err(resource_not_found_err!(
                        BranchVersion,
                        (branch, hash)
                    ))?,
            )
        } else {
            None
        };

        Ok(Self {
            uuid: BranchUuid::new(),
            project_id,
            name,
            slug,
            start_point_id,
            created: timestamp,
            modified: timestamp,
        })
    }

    pub fn main(conn: &mut DbConnection, project_id: ProjectId) -> Result<Self, HttpError> {
        Self::from_json(conn, project_id, JsonNewBranch::main())
    }

    pub async fn start_point(
        &self,
        log: &Logger,
        context: &ApiContext,
        clone_thresholds: bool,
    ) -> Result<(), HttpError> {
        let Some(start_point_id) = self.start_point_id else {
            return Ok(());
        };
        let start_point = QueryBranchVersion::get(conn_lock!(context), start_point_id)?;
        let new_branch_id = QueryBranch::get_id(conn_lock!(context), self.uuid)?;

        self.clone_versions(context, &start_point, new_branch_id)
            .await?;

        if clone_thresholds {
            self.clone_thresholds(log, context, &start_point, new_branch_id)
                .await?;
        }

        Ok(())
    }

    async fn clone_versions(
        &self,
        context: &ApiContext,
        start_point: &QueryBranchVersion,
        new_branch_id: BranchId,
    ) -> Result<(), HttpError> {
        let start_point_version = QueryVersion::get(conn_lock!(context), start_point.version_id)?;
        // Get all prior versions (version number less than or equal to) for the start point branch
        let version_ids = schema::branch_version::table
            .inner_join(schema::version::table)
            .filter(schema::branch_version::branch_id.eq(start_point.branch_id))
            .filter(schema::version::number.le(start_point_version.number))
            .order(schema::version::number.desc())
            .select(schema::branch_version::version_id)
            .load::<VersionId>(conn_lock!(context))
            .map_err(resource_not_found_err!(
                BranchVersion,
                (start_point, start_point_version)
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
        log: &Logger,
        context: &ApiContext,
        start_point: &QueryBranchVersion,
        new_branch_id: BranchId,
    ) -> Result<(), HttpError> {
        // Get all thresholds for the start point branch
        let query_thresholds = schema::threshold::table
            .filter(schema::threshold::branch_id.eq(start_point.branch_id))
            .load::<QueryThreshold>(conn_lock!(context))
            .map_err(resource_not_found_err!(Threshold, start_point))?;

        // Add new branch to cloned thresholds with cloned current threshold model
        for query_threshold in query_thresholds {
            // Hold the database lock across the entire `clone_threshold` call
            if let Err(e) =
                self.clone_threshold(conn_lock!(context), new_branch_id, &query_threshold)
            {
                slog::warn!(log, "Failed to clone threshold: {e}");
            }
        }

        Ok(())
    }

    fn clone_threshold(
        &self,
        conn: &mut DbConnection,
        new_branch_id: BranchId,
        query_threshold: &QueryThreshold,
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

        // Get the new threshold
        let threshold_id = QueryThreshold::get_id(conn, insert_threshold.uuid)?;

        // Get the current threshold model
        let model_id = query_threshold.model_id()?;
        let query_model = schema::model::table
            .filter(schema::model::id.eq(model_id))
            .first::<QueryModel>(conn)
            .map_err(resource_not_found_err!(Model, query_threshold))?;

        // Clone the current threshold model
        let mut insert_model = InsertModel::from(query_model.clone());
        // Set the new model to the new threshold
        insert_model.threshold_id = threshold_id;
        // Create the new model for the new threshold
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
        let JsonUpdateBranch { name, slug, .. } = update;
        Self {
            name,
            slug,
            modified: DateTime::now(),
        }
    }
}
