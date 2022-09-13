use bencher_rbac::{
    user::{OrganizationRoles, ProjectRoles},
    User as RbacUser,
};
use tracing::error;

use bencher_json::jwt::JsonWebToken;
use diesel::{QueryDsl, RunQueryDsl, SqliteConnection};
use dropshot::RequestContext;

use crate::{
    diesel::ExpressionMethods,
    error::{api_error, auth_error, map_auth_error},
    schema::{self},
    util::Context,
    ApiError,
};

pub struct AuthUser {
    pub id: i32,
    pub rbac: RbacUser,
}

impl AuthUser {
    pub async fn new(rqctx: &RequestContext<Context>) -> Result<Self, ApiError> {
        let request = rqctx.request.lock().await;

        let headers = request
            .headers()
            .get("Authorization")
            .ok_or_else(auth_error!("Missing \"Authorization\" header."))?
            .to_str()
            .map_err(map_auth_error!("Invalid \"Authorization\" header."))?;
        let (_, token) = headers
            .split_once("Bearer ")
            .ok_or_else(auth_error!("Missing \"Authorization\" Bearer."))?;
        let jwt: JsonWebToken = token.trim().to_string().into();

        let context = &mut *rqctx.context().lock().await;
        let token_data = jwt
            .validate_user(&context.secret_key)
            .map_err(map_auth_error!("Invalid JWT (JSON Web Token)."))?;

        let conn = &mut context.db_conn;
        let (user_id, admin, locked) = schema::user::table
            .filter(schema::user::email.eq(token_data.claims.email()))
            .select((schema::user::id, schema::user::admin, schema::user::locked))
            .first::<(i32, bool, bool)>(conn)
            .map_err(map_auth_error!("Invalid JWT (JSON Web Token)."))?;

        let rbac = RbacUser {
            admin,
            locked,
            organizations: Self::organization_roles(conn, user_id)?,
            projects: Self::project_roles(conn, user_id)?,
        };

        Ok(Self { id: user_id, rbac })
    }

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
            .map_err(api_error!())?
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

    pub fn project_roles(
        conn: &mut SqliteConnection,
        user_id: i32,
    ) -> Result<ProjectRoles, ApiError> {
        Ok(schema::project_role::table
            .filter(schema::project_role::user_id.eq(user_id))
            .order(schema::project_role::project_id)
            .select((schema::project_role::project_id, schema::project_role::role))
            .load::<(i32, String)>(conn)
            .map_err(api_error!())?
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
