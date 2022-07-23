use std::str::FromStr;

use bencher_json::NewTestbed;
use diesel::{
    Insertable,
    Queryable,
    SqliteConnection,
};
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use uuid::Uuid;

use crate::{
    db::{
        schema,
        schema::testbed as testbed_table,
    },
    diesel::{
        ExpressionMethods,
        QueryDsl,
        RunQueryDsl,
    },
};

#[derive(Queryable, Debug, Deserialize, Serialize, JsonSchema)]
pub struct QueryTestbed {
    pub id:         i32,
    pub uuid:       String,
    pub name:       String,
    pub os_name:    Option<String>,
    pub os_version: Option<String>,
    pub cpu:        Option<String>,
    pub ram:        Option<String>,
    pub disk:       Option<String>,
}

impl QueryTestbed {
    pub fn get_id(conn: &SqliteConnection, uuid: Option<Uuid>) -> Option<i32> {
        if let Some(uuid) = uuid {
            Some(
                schema::testbed::table
                    .filter(schema::testbed::uuid.eq(uuid.to_string()))
                    .select(schema::testbed::id)
                    .first(conn)
                    .unwrap(),
            )
        } else {
            None
        }
    }

    pub fn get_uuid(conn: &SqliteConnection, id: Option<i32>) -> Option<Uuid> {
        if let Some(id) = id {
            let uuid: String = schema::testbed::table
                .filter(schema::testbed::id.eq(id))
                .select(schema::testbed::uuid)
                .first(conn)
                .unwrap();
            let uuid = Uuid::from_str(&uuid).unwrap();
            Some(uuid)
        } else {
            None
        }
    }
}

#[derive(Insertable)]
#[table_name = "testbed_table"]
pub struct InsertTestbed {
    pub uuid:       String,
    pub name:       String,
    pub os_name:    Option<String>,
    pub os_version: Option<String>,
    pub cpu:        Option<String>,
    pub ram:        Option<String>,
    pub disk:       Option<String>,
}

impl InsertTestbed {
    pub fn new(testbed: NewTestbed) -> Self {
        let NewTestbed {
            name,
            os_name,
            os_version,
            cpu,
            ram,
            disk,
        } = testbed;
        Self {
            uuid: Uuid::new_v4().to_string(),
            name,
            os_name,
            os_version,
            cpu,
            ram,
            disk,
        }
    }
}
