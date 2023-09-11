use bencher_json::project::JsonProjectPermission;
use diesel::{Insertable, Queryable};

use crate::{model::user::UserId, schema::project_role as project_role_table};

use super::ProjectId;

crate::util::typed_id::typed_id!(ProjectRoleId);

#[derive(Insertable)]
#[diesel(table_name = project_role_table)]
pub struct InsertProjectRole {
    pub user_id: UserId,
    pub project_id: ProjectId,
    pub role: String,
    pub created: i64,
    pub modified: i64,
}

#[derive(Queryable)]
pub struct QueryProjectRole {
    pub id: ProjectRoleId,
    pub user_id: UserId,
    pub project_id: ProjectId,
    pub role: String,
    pub created: i64,
    pub modified: i64,
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

impl From<JsonProjectPermission> for Permission {
    fn from(permission: JsonProjectPermission) -> Self {
        match permission {
            JsonProjectPermission::View => Self::View,
            JsonProjectPermission::Create => Self::Create,
            JsonProjectPermission::Edit => Self::Edit,
            JsonProjectPermission::Delete => Self::Delete,
            JsonProjectPermission::Manage => Self::Manage,
            JsonProjectPermission::ViewRole => Self::ViewRole,
            JsonProjectPermission::CreateRole => Self::CreateRole,
            JsonProjectPermission::EditRole => Self::EditRole,
            JsonProjectPermission::DeleteRole => Self::DeleteRole,
        }
    }
}

impl From<Permission> for bencher_rbac::project::Permission {
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
