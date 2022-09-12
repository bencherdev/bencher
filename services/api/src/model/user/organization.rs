use crate::{
    error::query_error,
    schema::{self, organization_role as organization_role_table},
    ApiError,
};
use bencher_rbac::user::OrganizationRoles;
use diesel::{ExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl, SqliteConnection};
use tracing::error;

#[derive(Insertable)]
#[diesel(table_name = organization_role_table)]
pub struct InsertOrganizationRole {
    pub user_id: i32,
    pub organization_id: i32,
    pub role: String,
}

#[derive(Queryable)]
pub struct QueryOrganizationRole {
    pub id: i32,
    pub user_id: i32,
    pub organization_id: i32,
    pub role: String,
}

impl QueryOrganizationRole {
    pub fn organization_roles(
        conn: &mut SqliteConnection,
        user_id: i32,
    ) -> Result<OrganizationRoles, ApiError> {
        Ok(schema::organization_role::table
            .filter(schema::organization_role::user_id.eq(user_id))
            .order(schema::organization_role::organization_id)
            .select((
                schema::organization_role::organization_id,
                schema::organization_role::role,
            ))
            .load::<(i32, String)>(conn)
            .map_err(query_error!())?
            .into_iter()
            .filter_map(|(org_id, role)| match role.parse() {
                Ok(role) => Some((org_id.to_string(), role)),
                Err(e) => {
                    error!("Failed to parse organization role {role}: {e}");
                    debug_assert!(false, "Failed to parse organization role {role}: {e}");
                    None
                },
            })
            .collect())
    }
}
