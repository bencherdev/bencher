use bencher_json::report::JsonResource;
use diesel::{ExpressionMethods, Insertable, QueryDsl, RunQueryDsl, SqliteConnection};
use dropshot::HttpError;
use uuid::Uuid;

use crate::{schema, schema::resource as resource_table, util::http_error};

#[derive(Queryable, Debug)]
pub struct QueryResource {
    pub id: i32,
    pub uuid: String,
    pub min: f64,
    pub max: f64,
    pub avg: f64,
}

impl QueryResource {
    pub fn to_json(self) -> JsonResource {
        let Self {
            id: _,
            uuid: _,
            min,
            max,
            avg,
        } = self;
        JsonResource {
            min: min.into(),
            max: max.into(),
            avg: avg.into(),
        }
    }
}

#[derive(Insertable)]
#[diesel(table_name = resource_table)]
pub struct InsertResource {
    pub uuid: String,
    pub min: f64,
    pub max: f64,
    pub avg: f64,
}

impl From<JsonResource> for InsertResource {
    fn from(resource: JsonResource) -> Self {
        let JsonResource { min, max, avg } = resource;
        Self {
            uuid: Uuid::new_v4().to_string(),
            min: min.into(),
            max: max.into(),
            avg: avg.into(),
        }
    }
}

impl InsertResource {
    pub fn map_json(
        conn: &mut SqliteConnection,
        resource: Option<JsonResource>,
    ) -> Result<Option<i32>, HttpError> {
        Ok(if let Some(json_min_max_avg) = resource {
            let insert_min_max_avg: InsertResource = json_min_max_avg.into();
            diesel::insert_into(schema::resource::table)
                .values(&insert_min_max_avg)
                .execute(conn)
                .map_err(|_| http_error!("Failed to create benchmark data."))?;

            Some(
                schema::resource::table
                    .filter(schema::resource::uuid.eq(&insert_min_max_avg.uuid))
                    .select(schema::resource::id)
                    .first::<i32>(conn)
                    .map_err(|_| http_error!("Failed to create benchmark data."))?,
            )
        } else {
            None
        })
    }
}
