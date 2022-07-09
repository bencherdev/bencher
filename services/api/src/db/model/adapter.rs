use diesel::{
    QueryDsl,
    Queryable,
    RunQueryDsl,
    SqliteConnection,
};
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};

use crate::{
    db::schema,
    diesel::ExpressionMethods,
};

#[derive(Queryable, Debug, Deserialize, Serialize, JsonSchema)]
pub struct QueryAdapter {
    pub id:   i32,
    pub uuid: String,
    pub name: String,
}

impl QueryAdapter {
    pub fn get_id(conn: &SqliteConnection, name: String) -> i32 {
        schema::adapter::table
            .filter(schema::adapter::name.eq(name))
            .select(schema::adapter::id)
            .first(conn)
            .unwrap()
    }

    pub fn get_uuid(conn: &SqliteConnection, id: i32) -> String {
        schema::adapter::table
            .filter(schema::adapter::id.eq(id))
            .select(schema::adapter::uuid)
            .first(conn)
            .unwrap()
    }
}
