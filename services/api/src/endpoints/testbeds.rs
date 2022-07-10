use std::{
    str::FromStr,
    sync::Arc,
};

use diesel::{
    QueryDsl,
    RunQueryDsl,
    SqliteConnection,
};
use dropshot::{
    endpoint,
    HttpError,
    HttpResponseHeaders,
    HttpResponseOk,
    Path,
    RequestContext,
    TypedBody,
};
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    db::{
        model::testbed::QueryTestbed,
        schema,
    },
    diesel::ExpressionMethods,
    util::headers::CorsHeaders,
};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct Testbed {
    pub uuid:       Uuid,
    pub name:       String,
    pub os_name:    Option<String>,
    pub os_version: Option<String>,
    pub cpu:        Option<String>,
    pub ram:        Option<String>,
    pub disk:       Option<String>,
}

impl From<QueryTestbed> for Testbed {
    fn from(testbed: QueryTestbed) -> Self {
        let QueryTestbed {
            id: _,
            uuid,
            name,
            os_name,
            os_version,
            cpu,
            ram,
            disk,
        } = testbed;
        Self {
            uuid: Uuid::from_str(&uuid).unwrap(),
            name,
            os_name,
            os_version,
            cpu,
            ram,
            disk,
        }
    }
}

#[endpoint {
    method = GET,
    path = "/v0/testbeds",
    tags = ["testbeds"]
}]
pub async fn api_get_testbeds(
    rqctx: Arc<RequestContext<Mutex<SqliteConnection>>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Vec<Testbed>>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();

    let conn = db_connection.lock().await;
    let testbeds: Vec<Testbed> = schema::testbed::table
        .load::<QueryTestbed>(&*conn)
        .expect("Error loading tesbeds.")
        .into_iter()
        .map(Into::into)
        .collect();

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(testbeds),
        CorsHeaders::new_origin_all("GET".into(), "Content-Type".into()),
    ))
}

#[derive(Deserialize, JsonSchema)]
pub struct PathParams {
    pub testbed_uuid: Uuid,
}

#[endpoint {
    method = GET,
    path = "/v0/testbeds/{testbed_uuid}",
    tags = ["testbeds"]
}]
pub async fn api_get_testbed(
    rqctx: Arc<RequestContext<Mutex<SqliteConnection>>>,
    path_params: Path<PathParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Testbed>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();

    let path_params = path_params.into_inner();
    let conn = db_connection.lock().await;
    let adapter = schema::testbed::table
        .filter(schema::testbed::uuid.eq(path_params.testbed_uuid.to_string()))
        .first::<QueryTestbed>(&*conn)
        .unwrap()
        .into();

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(adapter),
        CorsHeaders::new_origin_all("GET".into(), "Content-Type".into()),
    ))
}

// #[endpoint {
//     method = POST,
//     path = "/v0/testbeds",
//     tags = ["testbeds"]
// }]
// pub async fn api_post_testbed(
//     rqctx: Arc<RequestContext<Mutex<SqliteConnection>>>,
//     body: TypedBody<JsonTestbed>,
// ) -> Result<HttpResponseAccepted<()>, HttpError> {
//     let db_connection = rqctx.context();

//     let json_report = body.into_inner();
//     let conn = db_connection.lock().await;
//     let insert_report = InsertReport::new(&*conn, json_report);
//     diesel::insert_into(schema::report::table)
//         .values(&insert_report)
//         .execute(&*conn)
//         .expect("Error saving new report to database.");

//     Ok(HttpResponseAccepted(()))
// }
