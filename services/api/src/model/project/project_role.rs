use diesel::{Insertable, Queryable};

use crate::schema::project_role as project_role_table;

#[derive(Insertable)]
#[diesel(table_name = project_role_table)]
pub struct InsertProjectRole {
    pub user_id: i32,
    pub project_id: i32,
    pub role: String,
    pub created: i64,
    pub modified: i64,
}

#[derive(Queryable)]
pub struct QueryProjectRole {
    pub id: i32,
    pub user_id: i32,
    pub project_id: i32,
    pub role: String,
    pub created: i64,
    pub modified: i64,
}
