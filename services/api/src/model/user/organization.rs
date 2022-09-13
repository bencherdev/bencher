use crate::schema::organization_role as organization_role_table;
use diesel::{Insertable, Queryable};

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
