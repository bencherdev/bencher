use bencher_rbac::user::ProjectRoles;
use diesel::{ExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl, SqliteConnection};
use tracing::error;

use crate::{
    error::query_error,
    schema::{self, project_role as project_role_table},
    ApiError,
};

#[derive(Insertable)]
#[diesel(table_name = project_role_table)]
pub struct InsertProjectRole {
    pub user_id: i32,
    pub project_id: i32,
    pub role: String,
}

#[derive(Queryable)]
pub struct QueryProjectRole {
    pub id: i32,
    pub user_id: i32,
    pub project_id: i32,
    pub role: String,
}

impl QueryProjectRole {
    pub fn project_roles(
        conn: &mut SqliteConnection,
        user_id: i32,
    ) -> Result<ProjectRoles, ApiError> {
        Ok(schema::project_role::table
            .filter(schema::project_role::user_id.eq(user_id))
            .order(schema::project_role::project_id)
            .select((schema::project_role::project_id, schema::project_role::role))
            .load::<(i32, String)>(conn)
            .map_err(query_error!())?
            .into_iter()
            .filter_map(|(proj_id, role)| match role.parse() {
                Ok(role) => Some((proj_id.to_string(), role)),
                Err(e) => {
                    error!("Failed to parse project role {role}: {e}");
                    debug_assert!(false, "Failed to parse project role {role}: {e}");
                    None
                },
            })
            .collect())
    }
}
