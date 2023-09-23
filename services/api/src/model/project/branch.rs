use std::str::FromStr;

use bencher_json::{
    project::branch::{
        JsonBranchVersion, JsonStartPoint, JsonUpdateBranch, JsonVersion, BRANCH_MAIN_STR,
    },
    BranchName, GitHash, JsonBranch, JsonNewBranch, ResourceId, Slug,
};
use chrono::Utc;
use diesel::{ExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl};
use dropshot::HttpError;
use uuid::Uuid;

use super::{
    branch_version::InsertBranchVersion,
    threshold::statistic::{InsertStatistic, QueryStatistic},
    version::{QueryVersion, VersionId},
    ProjectId, QueryProject,
};
use crate::{
    context::DbConnection,
    error::resource_not_found_err,
    model::project::threshold::{InsertThreshold, QueryThreshold},
    schema,
    schema::branch as branch_table,
    util::{
        query::{fn_get, fn_get_id},
        resource_id::fn_resource_id,
        slug::unwrap_child_slug,
        to_date_time,
    },
    ApiError,
};

crate::util::typed_id::typed_id!(BranchId);

fn_resource_id!(branch);

#[derive(Queryable, Identifiable, Associations)]
#[diesel(table_name = branch_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
pub struct QueryBranch {
    pub id: BranchId,
    pub uuid: String,
    pub project_id: ProjectId,
    pub name: String,
    pub slug: String,
    pub created: i64,
    pub modified: i64,
}

impl QueryBranch {
    fn_get!(branch);
    fn_get_id!(branch, BranchId);

    pub fn get_uuid(conn: &mut DbConnection, id: BranchId) -> Result<Uuid, ApiError> {
        let uuid: String = schema::branch::table
            .filter(schema::branch::id.eq(id))
            .select(schema::branch::uuid)
            .first(conn)
            .map_err(ApiError::from)?;
        Uuid::from_str(&uuid).map_err(ApiError::from)
    }

    pub fn from_uuid(
        conn: &mut DbConnection,
        project_id: ProjectId,
        uuid: Uuid,
    ) -> Result<Self, ApiError> {
        schema::branch::table
            .filter(schema::branch::project_id.eq(project_id))
            .filter(schema::branch::uuid.eq(uuid.to_string()))
            .first::<Self>(conn)
            .map_err(ApiError::from)
    }

    pub fn from_resource_id(
        conn: &mut DbConnection,
        project_id: ProjectId,
        branch: &ResourceId,
    ) -> Result<Self, HttpError> {
        schema::branch::table
            .filter(schema::branch::project_id.eq(project_id))
            .filter(resource_id(branch)?)
            .first::<Self>(conn)
            .map_err(resource_not_found_err!(Branch, branch.clone()))
    }

    pub fn get_branch_version_json(
        conn: &mut DbConnection,
        branch_id: BranchId,
        version_id: VersionId,
    ) -> Result<JsonBranchVersion, ApiError> {
        let JsonBranch {
            uuid,
            project,
            name,
            slug,
            created,
            modified,
        } = Self::get(conn, branch_id)?.into_json(conn)?;
        let QueryVersion { number, hash, .. } = QueryVersion::get(conn, version_id)?;
        Ok(JsonBranchVersion {
            uuid,
            project,
            name,
            slug,
            version: JsonVersion {
                number: u32::try_from(number).map_err(ApiError::from)?,
                hash: if let Some(version_hash) = hash.as_deref() {
                    Some(GitHash::from_str(version_hash)?)
                } else {
                    None
                },
            },
            created,
            modified,
        })
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonBranch, ApiError> {
        let Self {
            uuid,
            project_id,
            name,
            slug,
            created,
            modified,
            ..
        } = self;
        Ok(JsonBranch {
            uuid: Uuid::from_str(&uuid).map_err(ApiError::from)?,
            project: QueryProject::get_uuid(conn, project_id)?,
            name: BranchName::from_str(&name).map_err(ApiError::from)?,
            slug: Slug::from_str(&slug).map_err(ApiError::from)?,
            created: to_date_time(created).map_err(ApiError::from)?,
            modified: to_date_time(modified).map_err(ApiError::from)?,
        })
    }

    pub fn is_system(&self) -> bool {
        matches!(self.name.as_ref(), BRANCH_MAIN_STR)
            || matches!(self.slug.as_ref(), BRANCH_MAIN_STR)
    }
}

#[derive(Insertable)]
#[diesel(table_name = branch_table)]
pub struct InsertBranch {
    pub uuid: String,
    pub project_id: ProjectId,
    pub name: String,
    pub slug: String,
    pub created: i64,
    pub modified: i64,
}

impl InsertBranch {
    pub fn from_json(
        conn: &mut DbConnection,
        project_id: ProjectId,
        branch: JsonNewBranch,
    ) -> Self {
        let JsonNewBranch { name, slug, .. } = branch;
        let slug = unwrap_child_slug!(conn, project_id, name.as_ref(), slug, branch, QueryBranch);
        let timestamp = Utc::now().timestamp();
        Self {
            uuid: Uuid::new_v4().to_string(),
            project_id,
            name: name.into(),
            slug,
            created: timestamp,
            modified: timestamp,
        }
    }

    pub fn main(conn: &mut DbConnection, project_id: ProjectId) -> Self {
        Self::from_json(conn, project_id, JsonNewBranch::main())
    }

    pub fn start_point(
        &self,
        conn: &mut DbConnection,
        start_point: &JsonStartPoint,
    ) -> Result<(), ApiError> {
        let JsonStartPoint { branch, thresholds } = start_point;

        let start_point_branch_id =
            QueryBranch::from_resource_id(conn, self.project_id, branch)?.id;
        let new_branch_id = QueryBranch::get_id(conn, &self.uuid)?;

        // Get all versions for the start point branch
        let version_ids = schema::branch_version::table
            .filter(schema::branch_version::branch_id.eq(start_point_branch_id))
            .select(schema::branch_version::version_id)
            .load::<VersionId>(conn)?;

        // Add new branch to all start point branch versions
        for version_id in version_ids {
            let insert_branch_version = InsertBranchVersion {
                branch_id: new_branch_id,
                version_id,
            };

            diesel::insert_into(schema::branch_version::table)
                .values(&insert_branch_version)
                .execute(conn)
                .map_err(ApiError::from)?;
        }

        if let Some(true) = thresholds {
            // Get all thresholds for the start point branch
            let query_thresholds = schema::threshold::table
                .filter(schema::threshold::branch_id.eq(start_point_branch_id))
                .load::<QueryThreshold>(conn)?;

            // Add new branch to cloned thresholds with cloned statistics
            for query_threshold in query_thresholds {
                // Clone the threshold for the new branch
                let insert_threshold = InsertThreshold::new(
                    self.project_id,
                    query_threshold.metric_kind_id,
                    new_branch_id,
                    query_threshold.testbed_id,
                );

                // Create the new threshold
                diesel::insert_into(schema::threshold::table)
                    .values(&insert_threshold)
                    .execute(conn)
                    .map_err(ApiError::from)?;

                // If there is a statistic, clone that too
                let Some(statistic_id) = query_threshold.statistic_id else {
                    continue;
                };

                // Get the new threshold
                let threshold_id = QueryThreshold::get_id(conn, &insert_threshold.uuid)?;

                // Get the current threshold statistic
                let query_statistic = schema::statistic::table
                    .filter(schema::statistic::id.eq(statistic_id))
                    .first::<QueryStatistic>(conn)?;

                // Clone the current threshold statistic
                let mut insert_statistic = InsertStatistic::from(query_statistic);
                // For the new threshold
                insert_statistic.threshold_id = threshold_id;
                diesel::insert_into(schema::statistic::table)
                    .values(&insert_statistic)
                    .execute(conn)
                    .map_err(ApiError::from)?;

                // Get the new threshold statistic
                let statistic_id = QueryStatistic::get_id(conn, &insert_statistic.uuid)?;

                // Set the new statistic for the new threshold
                diesel::update(
                    schema::threshold::table.filter(schema::threshold::id.eq(threshold_id)),
                )
                .set(schema::threshold::statistic_id.eq(statistic_id))
                .execute(conn)
                .map_err(ApiError::from)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = branch_table)]
pub struct UpdateBranch {
    pub name: Option<String>,
    pub slug: Option<String>,
    pub modified: i64,
}

impl From<JsonUpdateBranch> for UpdateBranch {
    fn from(update: JsonUpdateBranch) -> Self {
        let JsonUpdateBranch { name, slug } = update;
        Self {
            name: name.map(Into::into),
            slug: slug.map(Into::into),
            modified: Utc::now().timestamp(),
        }
    }
}
