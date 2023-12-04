use bencher_json::{
    project::branch::{JsonBranchVersion, JsonStartPoint, JsonUpdateBranch, BRANCH_MAIN_STR},
    BranchName, BranchUuid, DateTime, JsonBranch, JsonNewBranch, ResourceId, Slug,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use super::{
    branch_version::InsertBranchVersion,
    threshold::statistic::{InsertStatistic, QueryStatistic},
    version::{QueryVersion, VersionId},
    ProjectId, QueryProject,
};
use crate::{
    context::DbConnection,
    error::{assert_parentage, resource_conflict_err, resource_not_found_err, BencherResource},
    model::project::threshold::{InsertThreshold, QueryThreshold},
    schema,
    schema::branch as branch_table,
    util::{
        fn_get::{fn_from_uuid, fn_get, fn_get_id, fn_get_uuid},
        resource_id::{fn_from_resource_id, fn_resource_id},
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
    fn_resource_id!(branch);
    fn_from_resource_id!(branch, Branch);

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

    pub fn start_point(
        &self,
        conn: &mut DbConnection,
        start_point: &JsonStartPoint,
    ) -> Result<(), HttpError> {
        let JsonStartPoint { branch, thresholds } = start_point;

        let start_point_branch_id =
            QueryBranch::from_resource_id(conn, self.project_id, branch)?.id;
        let new_branch_id = QueryBranch::get_id(conn, self.uuid)?;

        // Get all versions for the start point branch
        let version_ids = schema::branch_version::table
            .filter(schema::branch_version::branch_id.eq(start_point_branch_id))
            .select(schema::branch_version::version_id)
            .load::<VersionId>(conn)
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
                .execute(conn)
                .map_err(resource_conflict_err!(BranchVersion, insert_branch_version))?;
        }

        if let Some(true) = thresholds {
            // Get all thresholds for the start point branch
            let query_thresholds = schema::threshold::table
                .filter(schema::threshold::branch_id.eq(start_point_branch_id))
                .load::<QueryThreshold>(conn)
                .map_err(resource_not_found_err!(Threshold, start_point_branch_id))?;

            // Add new branch to cloned thresholds with cloned statistics
            for query_threshold in query_thresholds {
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

                // If there is a statistic, clone that too
                let Some(statistic_id) = query_threshold.statistic_id else {
                    continue;
                };

                // Get the new threshold
                let threshold_id = QueryThreshold::get_id(conn, insert_threshold.uuid)?;

                // Get the current threshold statistic
                let query_statistic = schema::statistic::table
                    .filter(schema::statistic::id.eq(statistic_id))
                    .first::<QueryStatistic>(conn)
                    .map_err(resource_not_found_err!(Statistic, query_threshold))?;

                // Clone the current threshold statistic
                let mut insert_statistic = InsertStatistic::from(query_statistic.clone());
                // For the new threshold
                insert_statistic.threshold_id = threshold_id;
                diesel::insert_into(schema::statistic::table)
                    .values(&insert_statistic)
                    .execute(conn)
                    .map_err(resource_conflict_err!(Statistic, insert_statistic))?;

                // Get the new threshold statistic
                let statistic_id = QueryStatistic::get_id(conn, insert_statistic.uuid)?;

                // Set the new statistic for the new threshold
                diesel::update(
                    schema::threshold::table.filter(schema::threshold::id.eq(threshold_id)),
                )
                .set(schema::threshold::statistic_id.eq(statistic_id))
                .execute(conn)
                .map_err(resource_conflict_err!(
                    Threshold,
                    (&query_threshold, &query_statistic)
                ))?;
            }
        }

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
