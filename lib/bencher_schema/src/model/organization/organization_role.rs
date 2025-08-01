use bencher_json::{
    DateTime, Jwt,
    organization::{OrganizationPermission, member::OrganizationRole},
};
use bencher_token::TokenKey;
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use super::{OrganizationId, QueryOrganization};
use crate::{
    context::DbConnection,
    error::unauthorized_error,
    model::user::{QueryUser, UserId},
    resource_not_found_err,
    schema::{self, organization_role as organization_role_table},
};

crate::macros::typed_id::typed_id!(OrganizationRoleId);

#[derive(diesel::Queryable)]
pub struct QueryOrganizationRole {
    pub id: OrganizationRoleId,
    pub user_id: UserId,
    pub organization_id: OrganizationId,
    pub role: OrganizationRole,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryOrganizationRole {
    pub fn count(
        conn: &mut DbConnection,
        organization_id: OrganizationId,
    ) -> Result<i64, HttpError> {
        schema::organization_role::table
            .filter(schema::organization_role::organization_id.eq(&organization_id))
            .count()
            .get_result(conn)
            .map_err(resource_not_found_err!(OrganizationRole, organization_id))
    }

    pub fn claimed_at(
        conn: &mut DbConnection,
        organization_id: OrganizationId,
    ) -> Result<DateTime, HttpError> {
        schema::organization_role::table
            .filter(schema::organization_role::organization_id.eq(&organization_id))
            .select(schema::organization_role::created)
            .order(schema::organization_role::created.asc())
            .first(conn)
            .map_err(resource_not_found_err!(OrganizationRole, organization_id))
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = organization_role_table)]
pub struct InsertOrganizationRole {
    pub user_id: UserId,
    pub organization_id: OrganizationId,
    pub role: OrganizationRole,
    pub created: DateTime,
    pub modified: DateTime,
}

impl InsertOrganizationRole {
    pub fn from_jwt(
        conn: &mut DbConnection,
        token_key: &TokenKey,
        invite: &Jwt,
        user_id: UserId,
    ) -> Result<Self, HttpError> {
        // Validate the invite JWT
        let claims = token_key
            .validate_invite(invite)
            .map_err(unauthorized_error)?;

        let email = claims.email();
        // Make sure the email in the invite is the same as the email associated with the user
        let email_user_id = QueryUser::get_id_from_email(conn, email)?;
        if user_id != email_user_id {
            return Err(unauthorized_error(format!(
                "Invitation email ({email}) is connected to user {email_user_id} which doesn't match {user_id}"
            )));
        }

        let timestamp = DateTime::now();
        Ok(InsertOrganizationRole {
            user_id,
            organization_id: QueryOrganization::get_id(conn, claims.org.uuid)?,
            role: claims.org.role,
            created: timestamp,
            modified: timestamp,
        })
    }
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

impl From<OrganizationPermission> for Permission {
    fn from(permission: OrganizationPermission) -> Self {
        match permission {
            OrganizationPermission::View => Self::View,
            OrganizationPermission::Create => Self::Create,
            OrganizationPermission::Edit => Self::Edit,
            OrganizationPermission::Delete => Self::Delete,
            OrganizationPermission::Manage => Self::Manage,
            OrganizationPermission::ViewRole => Self::ViewRole,
            OrganizationPermission::CreateRole => Self::CreateRole,
            OrganizationPermission::EditRole => Self::EditRole,
            OrganizationPermission::DeleteRole => Self::DeleteRole,
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
