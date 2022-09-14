use bencher_rbac::{
    user::{OrganizationRoles, ProjectRoles},
    Organization, Project, User as RbacUser,
};

use bencher_json::jwt::JsonWebToken;
use diesel::{JoinOnDsl, QueryDsl, RunQueryDsl, SqliteConnection};
use dropshot::RequestContext;
use oso::{PolarValue, ToPolar};

use crate::{
    diesel::ExpressionMethods,
    schema,
    util::{context::Rbac, error::debug_error, Context},
    ApiError,
};

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

const INVALID_JWT: &str = "Invalid JWT (JSON Web Token)";

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: i32,
    pub organizations: Vec<OrganizationId>,
    pub projects: Vec<ProjectId>,
    pub rbac: RbacUser,
}

#[derive(Debug, Clone, Copy)]
pub struct OrganizationId {
    pub id: i32,
}

impl From<i32> for OrganizationId {
    fn from(id: i32) -> Self {
        Self { id }
    }
}

impl From<OrganizationId> for Organization {
    fn from(org_id: OrganizationId) -> Self {
        Self {
            uuid: org_id.id.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ProjectId {
    pub id: i32,
    pub organization_id: i32,
}

impl From<ProjectId> for Project {
    fn from(proj_id: ProjectId) -> Self {
        Self {
            uuid: proj_id.id.to_string(),
            parent: proj_id.organization_id.to_string(),
        }
    }
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
        let (user_id, admin, locked) = schema::user::table
            .filter(schema::user::email.eq(token_data.claims.email()))
            .select((schema::user::id, schema::user::admin, schema::user::locked))
            .first::<(i32, bool, bool)>(conn)
            .map_err(map_auth_error!(INVALID_JWT))?;

        let (org_ids, org_roles) = Self::organization_roles(conn, user_id)?;
        let (proj_ids, proj_roles) = Self::project_roles(conn, user_id)?;
        let rbac = RbacUser {
            admin,
            locked,
            organizations: org_roles,
            projects: proj_roles,
        };

        Ok(Self {
            id: user_id,
            organizations: org_ids,
            projects: proj_ids,
            rbac,
        })
    }

    fn organization_roles(
        conn: &mut SqliteConnection,
        user_id: i32,
    ) -> Result<(Vec<OrganizationId>, OrganizationRoles), ApiError> {
        let roles = schema::organization_role::table
            .filter(schema::organization_role::user_id.eq(user_id))
            .order(schema::organization_role::organization_id)
            .select((
                schema::organization_role::organization_id,
                schema::organization_role::role,
            ))
            .load::<(i32, String)>(conn)
            .map_err(map_auth_error!(INVALID_JWT))?;

        let ids = roles.iter().map(|(id, _)| (*id).into()).collect();
        let roles = roles
            .into_iter()
            .filter_map(|(id, role)| match role.parse() {
                Ok(role) => Some((id.to_string(), role)),
                Err(e) => {
                    debug_error!("Failed to parse organization role \"{role}\": {e}");
                    None
                },
            })
            .collect();

        Ok((ids, roles))
    }

    fn project_roles(
        conn: &mut SqliteConnection,
        user_id: i32,
    ) -> Result<(Vec<ProjectId>, ProjectRoles), ApiError> {
        let roles = schema::project_role::table
            .filter(schema::project_role::user_id.eq(user_id))
            .inner_join(
                schema::project::table.on(schema::project_role::project_id.eq(schema::project::id)),
            )
            .order(schema::project_role::project_id)
            .select((
                schema::project::organization_id,
                schema::project_role::project_id,
                schema::project_role::role,
            ))
            .load::<(i32, i32, String)>(conn)
            .map_err(map_auth_error!(INVALID_JWT))?;

        let ids = roles
            .iter()
            .map(|(org_id, id, _)| ProjectId {
                id: *id,
                organization_id: *org_id,
            })
            .collect();
        let roles = roles
            .into_iter()
            .filter_map(|(_, id, role)| match role.parse() {
                Ok(role) => Some((id.to_string(), role)),
                Err(e) => {
                    debug_error!("Failed to parse project role \"{role}\": {e}");
                    None
                },
            })
            .collect();

        Ok((ids, roles))
    }

    pub fn organizations(
        &self,
        rbac: &Rbac,
        action: bencher_rbac::organization::Permission,
    ) -> Vec<i32> {
        self.organizations
            .iter()
            .filter_map(|org_id| {
                if rbac.unwrap_is_allowed(self, action, Organization::from(*org_id)) {
                    Some(org_id.id)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn projects(&self, rbac: &Rbac, action: bencher_rbac::project::Permission) -> Vec<i32> {
        self.projects
            .iter()
            .filter_map(|proj_id| {
                if rbac.unwrap_is_allowed(self, action, Project::from(*proj_id)) {
                    Some(proj_id.id)
                } else {
                    None
                }
            })
            .collect()
    }
}

impl ToPolar for &AuthUser {
    fn to_polar(self) -> PolarValue {
        self.rbac.clone().to_polar()
    }
}
