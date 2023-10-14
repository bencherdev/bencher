use bencher_json::{
    project::{ProjectPermission, ProjectRole},
    DateTime,
};

use crate::{model::user::UserId, schema::project_role as project_role_table};

use super::ProjectId;

crate::util::typed_id::typed_id!(ProjectRoleId);

#[derive(diesel::Queryable)]
pub struct QueryProjectRole {
    pub id: ProjectRoleId,
    pub user_id: UserId,
    pub project_id: ProjectId,
    pub role: ProjectRole,
    pub created: DateTime,
    pub modified: DateTime,
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = project_role_table)]
pub struct InsertProjectRole {
    pub user_id: UserId,
    pub project_id: ProjectId,
    pub role: ProjectRole,
    pub created: DateTime,
    pub modified: DateTime,
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

impl From<ProjectPermission> for Permission {
    fn from(permission: ProjectPermission) -> Self {
        match permission {
            ProjectPermission::View => Self::View,
            ProjectPermission::Create => Self::Create,
            ProjectPermission::Edit => Self::Edit,
            ProjectPermission::Delete => Self::Delete,
            ProjectPermission::Manage => Self::Manage,
            ProjectPermission::ViewRole => Self::ViewRole,
            ProjectPermission::CreateRole => Self::CreateRole,
            ProjectPermission::EditRole => Self::EditRole,
            ProjectPermission::DeleteRole => Self::DeleteRole,
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
