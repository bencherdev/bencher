use bencher_json::ResourceId;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SqliteConnection};

use crate::{
    error::api_error,
    model::project::{branch::QueryBranch, testbed::QueryTestbed, QueryProject},
    schema, ApiError,
};

pub struct SameProject {
    pub project_id: i32,
    pub branch_id: i32,
    pub testbed_id: i32,
}

impl SameProject {
    pub fn validate(
        conn: &mut SqliteConnection,
        project: &ResourceId,
        branch: &ResourceId,
        testbed: &ResourceId,
    ) -> Result<Self, ApiError> {
        let project_id = QueryProject::from_resource_id(conn, project)?.id;
        let branch_id = QueryBranch::from_resource_id(conn, project_id, branch)?.id;
        let testbed_id = QueryTestbed::from_resource_id(conn, project_id, testbed)?.id;

        Ok(Self {
            project_id,
            branch_id,
            testbed_id,
        })
    }

    pub fn validate_ids(
        conn: &mut SqliteConnection,
        project_id: i32,
        branch_id: i32,
        testbed_id: i32,
    ) -> Result<(), ApiError> {
        let branch_project_id = schema::branch::table
            .filter(schema::branch::id.eq(branch_id))
            .select(schema::branch::project_id)
            .first::<i32>(conn)
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
            .first::<i32>(conn)
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
