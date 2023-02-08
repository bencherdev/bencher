use crate::{
    context::SecretKey,
    model::user::{
        auth::{auth_header_error, map_auth_header_error, INVALID_JWT},
        QueryUser,
    },
    schema::organization_role as organization_role_table,
    util::jwt::JsonWebToken,
    ApiError,
};
use bencher_json::{organization::JsonOrganizationPermission, Jwt};
use diesel::{Insertable, Queryable, SqliteConnection};

use super::QueryOrganization;

#[derive(Insertable)]
#[diesel(table_name = organization_role_table)]
pub struct InsertOrganizationRole {
    pub user_id: i32,
    pub organization_id: i32,
    pub role: String,
}

impl InsertOrganizationRole {
    pub fn from_jwt(
        conn: &mut SqliteConnection,
        invite: &Jwt,
        secret_key: &SecretKey,
        user_id: i32,
    ) -> Result<Self, ApiError> {
        // Validate the invite JWT
        let token_data = JsonWebToken::validate_invite(invite, &secret_key.decoding)
            .map_err(map_auth_header_error!(INVALID_JWT))?;

        // Make sure that there is an `org` field in the claims
        let org_claims = token_data
            .claims
            .org()
            .ok_or_else(auth_header_error!(INVALID_JWT))?;

        // Make sure the email in the invite is the same as the email associated with the user
        let email_user_id = QueryUser::get_id_from_email(conn, token_data.claims.email())?;
        if user_id != email_user_id {
            return Err(ApiError::InviteEmail {
                user_id,
                email: token_data.claims.email().into(),
                email_user_id,
            });
        }

        Ok(InsertOrganizationRole {
            user_id,
            organization_id: QueryOrganization::get_id(conn, &org_claims.uuid)?,
            role: org_claims.role.to_string(),
        })
    }
}

#[derive(Queryable)]
pub struct QueryOrganizationRole {
    pub id: i32,
    pub user_id: i32,
    pub organization_id: i32,
    pub role: String,
}

pub enum Permission {
    View,
    Create,
    Edit,
    Delete,
    Manage,
    ViewRole,
    CreateRole,
    EditRole,
    DeleteRole,
}

impl From<JsonOrganizationPermission> for Permission {
    fn from(permission: JsonOrganizationPermission) -> Self {
        match permission {
            JsonOrganizationPermission::View => Self::View,
            JsonOrganizationPermission::Create => Self::Create,
            JsonOrganizationPermission::Edit => Self::Edit,
            JsonOrganizationPermission::Delete => Self::Delete,
            JsonOrganizationPermission::Manage => Self::Manage,
            JsonOrganizationPermission::ViewRole => Self::ViewRole,
            JsonOrganizationPermission::CreateRole => Self::CreateRole,
            JsonOrganizationPermission::EditRole => Self::EditRole,
            JsonOrganizationPermission::DeleteRole => Self::DeleteRole,
        }
    }
}

impl From<Permission> for bencher_rbac::organization::Permission {
    fn from(permission: Permission) -> Self {
        match permission {
            Permission::View => Self::View,
            Permission::Create => Self::Create,
            Permission::Edit => Self::Edit,
            Permission::Delete => Self::Delete,
            Permission::Manage => Self::Manage,
            Permission::ViewRole => Self::ViewRole,
            Permission::CreateRole => Self::CreateRole,
            Permission::EditRole => Self::EditRole,
            Permission::DeleteRole => Self::DeleteRole,
        }
    }
}
