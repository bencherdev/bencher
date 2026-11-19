use bencher_json::{
    DateTime,
    organization::{OrganizationPermission, member::OrganizationRole},
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use super::OrganizationId;
use crate::{
    context::DbConnection,
    model::user::UserId,
    resource_conflict_err, resource_not_found_err,
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
    pub(crate) fn count_query(
        conn: &mut DbConnection,
        organization_id: OrganizationId,
    ) -> diesel::QueryResult<i64> {
        schema::organization_role::table
            .filter(schema::organization_role::organization_id.eq(&organization_id))
            .count()
            .get_result(conn)
    }

    pub fn count(
        conn: &mut DbConnection,
        organization_id: OrganizationId,
    ) -> Result<i64, HttpError> {
        Self::count_query(conn, organization_id)
            .map_err(resource_not_found_err!(OrganizationRole, organization_id))
    }

    /// Whether the user holds a role in the organization.
    pub fn exists(
        conn: &mut DbConnection,
        user_id: UserId,
        organization_id: OrganizationId,
    ) -> diesel::QueryResult<bool> {
        diesel::select(diesel::dsl::exists(
            schema::organization_role::table
                .filter(schema::organization_role::user_id.eq(user_id))
                .filter(schema::organization_role::organization_id.eq(organization_id)),
        ))
        .get_result(conn)
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
    pub fn insert(&self, conn: &mut DbConnection) -> Result<(), HttpError> {
        diesel::insert_into(schema::organization_role::table)
            .values(self)
            .execute(conn)
            .map(|_inserted| ())
            .map_err(resource_conflict_err!(OrganizationRole, self))
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
