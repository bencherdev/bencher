use std::ops::Deref;

use async_trait::async_trait;
#[cfg(feature = "plus")]
use bencher_json::system::payment::JsonCustomer;
use bencher_json::{Jwt, Sanitize};
use bencher_rbac::{
    Organization, Project, Server, User as RbacUser,
    server::Permission,
    user::{OrganizationRoles, ProjectRoles},
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::{
    ApiEndpointBodyContentType, ExtensionMode, ExtractorMetadata, HttpError, RequestContext,
    ServerContext, SharedExtractor,
};
use oso::{PolarValue, ToPolar};

use crate::{
    context::{ApiContext, DbConnection, Rbac},
    error::{BEARER_TOKEN_FORMAT, bad_request_error},
    model::{organization::OrganizationId, project::ProjectId},
    schema, try_conn,
};

use super::QueryUser;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user: QueryUser,
    pub organizations: Vec<OrganizationId>,
    pub projects: Vec<OrgProjectId>,
    pub rbac: RbacUser,
}

impl AuthUser {
    // This is required due to a limitation in `dropshot` where only four extractors are allowed.
    pub async fn new(rqctx: &RequestContext<ApiContext>) -> Result<Self, HttpError> {
        let bearer_token = BearerToken::from_request(rqctx).await?;
        Self::from_token(rqctx.context(), bearer_token).await
    }

    pub async fn from_token(
        context: &ApiContext,
        bearer_token: BearerToken,
    ) -> Result<Self, HttpError> {
        let claims = context
            .token_key
            .validate_client(&bearer_token)
            .map_err(|e| bad_request_error(format!("Failed to validate JSON Web Token: {e}")))?;
        let email = claims.email();

        // Hold the connection for all permissions related queries
        let conn = try_conn!(context);
        let query_user = QueryUser::get_with_email(conn, email)?;
        query_user.check_is_locked()?;
        #[cfg(feature = "plus")]
        context.rate_limiting.user_request(query_user.uuid)?;

        Self::load(conn, query_user)
    }

    pub fn reload(&self, conn: &mut DbConnection) -> Result<Self, HttpError> {
        Self::load(conn, self.user.clone())
    }

    fn load(conn: &mut DbConnection, query_user: QueryUser) -> Result<Self, HttpError> {
        let (org_ids, org_roles) = Self::organization_roles(conn, &query_user)?;
        let (proj_ids, proj_roles) = Self::project_roles(conn, &query_user)?;

        let rbac = RbacUser {
            admin: query_user.admin,
            locked: query_user.locked,
            organizations: org_roles,
            projects: proj_roles,
        };

        Ok(Self {
            user: query_user,
            organizations: org_ids,
            projects: proj_ids,
            rbac,
        })
    }

    fn organization_roles(
        conn: &mut DbConnection,
        query_user: &QueryUser,
    ) -> Result<(Vec<OrganizationId>, OrganizationRoles), HttpError> {
        let roles = schema::organization_role::table
            .filter(schema::organization_role::user_id.eq(query_user.id))
            .order(schema::organization_role::organization_id)
            .select((
                schema::organization_role::organization_id,
                schema::organization_role::role,
            ))
            .load::<(OrganizationId, String)>(conn)
            .map_err(|e| {
                crate::error::issue_error(
                    "User can't query organization roles",
                    &format!(
                        "My user ({email}) on Bencher failed to query organization roles.",
                        email = query_user.email
                    ),
                    e,
                )
            })?;

        let org_ids = roles.iter().map(|(org_id, _)| *org_id).collect();
        let roles = roles
            .into_iter()
            .filter_map(|(org_id, role)| match role.parse() {
                Ok(role) => Some((org_id.to_string(), role)),
                Err(e) => {
                    let _err = crate::error::issue_error(
                        "Failed to parse organization role",
                        &format!("My user ({email}) on Bencher has an invalid organization role ({role}).", email = query_user.email),
                        e,
                    );
                    None
                },
            })
            .collect();

        Ok((org_ids, roles))
    }

    fn project_roles(
        conn: &mut DbConnection,
        query_user: &QueryUser,
    ) -> Result<(Vec<OrgProjectId>, ProjectRoles), HttpError> {
        let roles = schema::project_role::table
            .filter(schema::project_role::user_id.eq(query_user.id))
            .inner_join(schema::project::table)
            .order(schema::project_role::project_id)
            .select((
                schema::project::organization_id,
                schema::project_role::project_id,
                schema::project_role::role,
            ))
            .load::<(OrganizationId, ProjectId, String)>(conn)
            .map_err(|e| {
                crate::error::issue_error(
                    "User can't query project roles",
                    &format!(
                        "My user ({email}) on Bencher failed to query project roles.",
                        email = query_user.email
                    ),
                    e,
                )
            })?;

        let ids = roles
            .iter()
            .map(|(org_id, project_id, _)| OrgProjectId {
                org_id: *org_id,
                project_id: *project_id,
            })
            .collect();
        let roles = roles
            .into_iter()
            .filter_map(|(_, id, role)| match role.parse() {
                Ok(role) => Some((id.to_string(), role)),
                Err(e) => {
                    let _err = crate::error::issue_error(
                        "Failed to parse project role",
                        &format!(
                            "My user ({email}) on Bencher has an invalid project role ({role}).",
                            email = query_user.email
                        ),
                        e,
                    );
                    None
                },
            })
            .collect();

        Ok((ids, roles))
    }

    pub fn is_admin(&self, rbac: &Rbac) -> bool {
        rbac.is_allowed_unwrap(self, Permission::Administer, Server {})
    }

    pub fn organizations(
        &self,
        rbac: &Rbac,
        action: bencher_rbac::organization::Permission,
    ) -> Vec<OrganizationId> {
        self.organizations
            .iter()
            .filter_map(|org_id| {
                rbac.is_allowed_unwrap(self, action, Organization::from(*org_id))
                    .then_some(*org_id)
            })
            .collect()
    }

    pub fn projects(
        &self,
        rbac: &Rbac,
        action: bencher_rbac::project::Permission,
    ) -> Vec<ProjectId> {
        self.projects
            .iter()
            .filter_map(|org_project_id| {
                rbac.is_allowed_unwrap(self, action, Project::from(*org_project_id))
                    .then_some(org_project_id.project_id)
            })
            .collect()
    }

    #[cfg(feature = "plus")]
    pub fn rate_limit_invites(&self, context: &ApiContext) -> Result<(), HttpError> {
        context.rate_limiting.user_invite(self.user.uuid)
    }

    #[cfg(feature = "plus")]
    pub fn to_customer(&self) -> JsonCustomer {
        JsonCustomer {
            uuid: self.user.uuid,
            name: self.user.name.clone().into(),
            email: self.user.email.clone(),
        }
    }
}

impl Deref for AuthUser {
    type Target = QueryUser;

    fn deref(&self) -> &Self::Target {
        &self.user
    }
}

impl Sanitize for AuthUser {
    fn sanitize(&mut self) {
        self.user.sanitize();
    }
}

// https://github.com/oxidecomputer/cio/blob/master/dropshot-verify-request/src/bearer.rs
pub struct BearerToken(Jwt);

impl From<Jwt> for BearerToken {
    fn from(jwt: Jwt) -> Self {
        Self(jwt)
    }
}

impl Deref for BearerToken {
    type Target = Jwt;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[async_trait]
impl SharedExtractor for BearerToken {
    async fn from_request<Context: ServerContext>(
        rqctx: &RequestContext<Context>,
    ) -> Result<Self, HttpError> {
        let headers = rqctx.request.headers();

        let Some(authorization) = headers.get("Authorization") else {
            return Err(bad_request_error(format!(
                "Request is missing \"Authorization\" header. {BEARER_TOKEN_FORMAT}"
            )));
        };
        let authorization_str = match authorization.to_str() {
            Ok(authorization_str) => authorization_str,
            Err(e) => {
                return Err(bad_request_error(format!(
                    "Request has an invalid \"Authorization\" header: {e}. {BEARER_TOKEN_FORMAT}"
                )));
            },
        };
        let Some(("Bearer", token)) = authorization_str.split_once(' ') else {
            return Err(bad_request_error(format!(
                "Request is missing \"Authorization\" Bearer. {BEARER_TOKEN_FORMAT}"
            )));
        };

        token
            .trim()
            .parse::<Jwt>()
            .map(Into::into)
            .map_err(|e| bad_request_error(format!("Malformed JSON Web Token: {e}")))
    }

    fn metadata(_body_content_type: ApiEndpointBodyContentType) -> ExtractorMetadata {
        ExtractorMetadata {
            extension_mode: ExtensionMode::None,
            parameters: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct OrgProjectId {
    pub org_id: OrganizationId,
    pub project_id: ProjectId,
}

impl From<OrgProjectId> for Project {
    fn from(org_project_id: OrgProjectId) -> Self {
        Self {
            organization_id: org_project_id.org_id.to_string(),
            id: org_project_id.project_id.to_string(),
        }
    }
}

impl ToPolar for &AuthUser {
    fn to_polar(self) -> PolarValue {
        self.rbac.clone().to_polar()
    }
}
