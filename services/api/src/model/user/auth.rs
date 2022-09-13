use bencher_rbac::{
    user::{OrganizationRoles, ProjectRoles},
    Organization, User as RbacUser,
};

use bencher_json::jwt::JsonWebToken;
use diesel::{QueryDsl, RunQueryDsl, SqliteConnection};
use dropshot::RequestContext;
use oso::{PolarValue, ToPolar};

use crate::{
    diesel::ExpressionMethods,
    schema,
    util::{context::Rbac, Context},
    ApiError,
};

const INVALID_JWT: &str = "Invalid JWT (JSON Web Token)";

macro_rules! auth_error {
    ($message:expr) => {
        || {
            tracing::info!($message);
            crate::error::ApiError::Auth($message.into())
        }
    };
}

macro_rules! map_auth_error {
    ($message:expr) => {
        |e| {
            tracing::info!("{}: {}", $message, e);
            crate::error::ApiError::Auth($message.into())
        }
    };
}

macro_rules! roles {
    ($conn:ident, $user_id:ident, $table:ident, $user_id_field:ident, $field:ident, $role_field:ident, $msg:expr) => {
        Ok(schema::$table::table
            .filter(schema::$table::$user_id_field.eq($user_id))
            .order(schema::$table::$field)
            .select((schema::$table::$field, schema::$table::$role_field))
            .load::<(i32, String)>($conn)
            .map_err(map_auth_error!(INVALID_JWT))?
            .into_iter()
            .filter_map(|(id, role)| match role.parse() {
                Ok(role) => Some((id.to_string(), role)),
                Err(e) => {
                    tracing::error!($msg, role, e);
                    debug_assert!(false, $msg, role, e);
                    None
                },
            })
            .collect())
    };
}

#[derive(Debug, Clone, Copy)]
pub struct AuthUser {
    pub id: i32,
    pub admin: bool,
    pub locked: bool,
}

impl AuthUser {
    pub async fn new(rqctx: &RequestContext<Context>) -> Result<Self, ApiError> {
        let request = rqctx.request.lock().await;

        let headers = request
            .headers()
            .get("Authorization")
            .ok_or_else(auth_error!("Missing \"Authorization\" header"))?
            .to_str()
            .map_err(map_auth_error!("Invalid \"Authorization\" header"))?;
        let (_, token) = headers
            .split_once("Bearer ")
            .ok_or_else(auth_error!("Missing \"Authorization\" Bearer"))?;
        let jwt: JsonWebToken = token.trim().to_string().into();

        let context = &mut *rqctx.context().lock().await;
        let token_data = jwt
            .validate_user(&context.secret_key)
            .map_err(map_auth_error!(INVALID_JWT))?;

        let conn = &mut context.db_conn;
        schema::user::table
            .filter(schema::user::email.eq(token_data.claims.email()))
            .select((schema::user::id, schema::user::admin, schema::user::locked))
            .first::<(i32, bool, bool)>(conn)
            .map(|(id, admin, locked)| Self { id, admin, locked })
            .map_err(map_auth_error!(INVALID_JWT))
    }

    fn into_rbac(self, conn: &mut SqliteConnection) -> Result<RbacUser, ApiError> {
        let AuthUser { id, admin, locked } = self;
        Ok(RbacUser {
            admin,
            locked,
            organizations: Self::organization_roles(conn, id)?,
            projects: Self::project_roles(conn, id)?,
        })
    }

    fn organization_roles(
        conn: &mut SqliteConnection,
        user_id: i32,
    ) -> Result<OrganizationRoles, ApiError> {
        roles!(
            conn,
            user_id,
            organization_role,
            user_id,
            organization_id,
            role,
            "Failed to parse organization role {}: {}"
        )
    }

    fn project_roles(conn: &mut SqliteConnection, user_id: i32) -> Result<ProjectRoles, ApiError> {
        roles!(
            conn,
            user_id,
            project_role,
            user_id,
            project_id,
            role,
            "Failed to parse project role {}: {}"
        )
    }

    pub fn organizations(
        &self,
        rbac: &Rbac,
        action: bencher_rbac::organization::Permission,
    ) -> Result<Vec<i32>, ApiError> {
        let mut ids = Vec::new();
        // for id in self.rbac.organizations.keys().cloned() {
        //     if rbac.unwrap_is_allowed(self, action, Organization { uuid: id }) {
        //         // ids.push(id.parse().unwrap())
        //     }
        // }
        Ok(ids)
    }
}
