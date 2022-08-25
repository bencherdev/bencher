use bencher_json::report::JsonMinMaxAvg;
use diesel::{
    Insertable,
    SqliteConnection,
};
use dropshot::HttpError;
use uuid::Uuid;

use crate::{
    db::{
        schema,
        schema::min_max_avg as min_max_avg_table,
    },
    diesel::{
        ExpressionMethods,
        QueryDsl,
        RunQueryDsl,
    },
    util::http_error,
};

#[derive(Queryable, Debug)]
pub struct QueryMinMaxAvg {
    pub id:   i32,
    pub uuid: String,
    pub min:  f64,
    pub max:  f64,
    pub avg:  f64,
}

impl QueryMinMaxAvg {
    pub fn to_json(self) -> JsonMinMaxAvg {
        let Self {
            id: _,
            uuid: _,
            min,
            max,
            avg,
        } = self;
        JsonMinMaxAvg {
            min: min.into(),
            max: max.into(),
            avg: avg.into(),
        }
    }
}

#[derive(Insertable)]
#[table_name = "min_max_avg_table"]
pub struct InsertMinMaxAvg {
    pub uuid: String,
    pub min:  f64,
    pub max:  f64,
    pub avg:  f64,
}

impl From<JsonMinMaxAvg> for InsertMinMaxAvg {
    fn from(min_max_avg: JsonMinMaxAvg) -> Self {
        let JsonMinMaxAvg { min, max, avg } = min_max_avg;
        Self {
            uuid: Uuid::new_v4().to_string(),
            min:  min.into(),
            max:  max.into(),
            avg:  avg.into(),
        }
    }
}

impl InsertMinMaxAvg {
    pub fn map_json(
        conn: &SqliteConnection,
        min_max_avg: Option<JsonMinMaxAvg>,
    ) -> Result<Option<i32>, HttpError> {
        Ok(if let Some(json_min_max_avg) = min_max_avg {
            let insert_min_max_avg: InsertMinMaxAvg = json_min_max_avg.into();
            diesel::insert_into(schema::min_max_avg::table)
                .values(&insert_min_max_avg)
                .execute(&*conn)
                .map_err(|_| http_error!("Failed to create benchmark data."))?;

            Some(
                schema::min_max_avg::table
                    .filter(schema::min_max_avg::uuid.eq(&insert_min_max_avg.uuid))
                    .select(schema::min_max_avg::id)
                    .first::<i32>(&*conn)
                    .map_err(|_| http_error!("Failed to create benchmark data."))?,
            )
        } else {
            None
        })
    }
}
