use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SqliteConnection};

use crate::{
    error::api_error,
    model::{branch::QueryBranch, testbed::QueryTestbed},
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
        branch: impl ToString,
        testbed: impl ToString,
    ) -> Result<Self, ApiError> {
        let branch_id = QueryBranch::get_id(conn, branch)?;
        let testbed_id = QueryTestbed::get_id(conn, testbed)?;

        let branch_project_id = schema::branch::table
            .filter(schema::branch::id.eq(branch_id))
            .select(schema::branch::project_id)
            .first::<i32>(conn)
            .map_err(api_error!())?;
        let testbed_project_id = schema::testbed::table
            .filter(schema::testbed::id.eq(testbed_id))
            .select(schema::testbed::project_id)
            .first::<i32>(conn)
            .map_err(api_error!())?;

        if branch_project_id == testbed_project_id {
            Ok(Self {
                project_id: branch_project_id,
                branch_id,
                testbed_id,
            })
        } else {
            Err(ApiError::BranchTestbedProject {
                branch_id,
                branch_project_id,
                testbed_id,
                testbed_project_id,
            })
        }
    }
}
