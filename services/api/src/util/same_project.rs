use bencher_json::ResourceId;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::api_error,
    model::project::{
        branch::{BranchId, QueryBranch},
        testbed::QueryTestbed,
        ProjectId, QueryProject,
    },
    schema, ApiError,
};

pub struct SameProject {
    pub project: QueryProject,
    pub branch_id: BranchId,
    pub testbed_id: i32,
}

impl SameProject {
    pub fn validate(
        conn: &mut DbConnection,
        project: &ResourceId,
        branch: &ResourceId,
        testbed: &ResourceId,
    ) -> Result<Self, HttpError> {
        let project = QueryProject::from_resource_id(conn, project)?;
        let branch_id = QueryBranch::from_resource_id(conn, project.id, branch)?.id;
        let testbed_id = QueryTestbed::from_resource_id(conn, project.id, testbed)?.id;

        Ok(Self {
            project,
            branch_id,
            testbed_id,
        })
    }

    pub fn validate_ids(
        conn: &mut DbConnection,
        project_id: ProjectId,
        branch_id: BranchId,
        testbed_id: i32,
    ) -> Result<(), ApiError> {
        let branch_project_id = schema::branch::table
            .filter(schema::branch::id.eq(branch_id))
            .select(schema::branch::project_id)
            .first::<ProjectId>(conn)
            .map_err(api_error!())?;
        if project_id != branch_project_id {
            return Err(ApiError::BranchProject {
                project_id,
                branch_id,
                branch_project_id,
            });
        }

        let testbed_project_id = schema::testbed::table
            .filter(schema::testbed::id.eq(testbed_id))
            .select(schema::testbed::project_id)
            .first::<ProjectId>(conn)
            .map_err(api_error!())?;
        if project_id != testbed_project_id {
            return Err(ApiError::TestbedProject {
                project_id,
                testbed_id,
                testbed_project_id,
            });
        }

        Ok(())
    }
}
