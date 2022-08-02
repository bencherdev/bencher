use std::{
    str::FromStr,
    sync::Arc,
};

use bencher_json::{
    JsonNewTestbed,
    JsonTestbed,
};
use diesel::{
    QueryDsl,
    RunQueryDsl,
};
use dropshot::{
    endpoint,
    HttpError,
    HttpResponseAccepted,
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
use uuid::Uuid;

use crate::{
    db::{
        model::testbed::{
            InsertTestbed,
            QueryTestbed,
        },
        schema,
    },
    diesel::ExpressionMethods,
    util::{
        headers::CorsHeaders,
        http_error,
        Context,
    },
};

#[endpoint {
    method = GET,
    path = "/v0/testbeds",
    tags = ["testbeds"]
}]
pub async fn api_get_testbeds(
    rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Vec<JsonTestbed>>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();

    let conn = db_connection.lock().await;
    let json: Vec<JsonTestbed> = schema::testbed::table
        .load::<QueryTestbed>(&*conn)
        .map_err(|_| http_error!("Failed to get testbeds."))?
        .into_iter()
        .filter_map(|query| query.to_json(&*conn).ok())
        .collect();

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("GET".into()),
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
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<PathParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<JsonTestbed>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();

    let path_params = path_params.into_inner();
    let conn = db_connection.lock().await;
    let query = schema::testbed::table
        .filter(schema::testbed::uuid.eq(&path_params.testbed_uuid.to_string()))
        .first::<QueryTestbed>(&*conn)
        .map_err(|_| http_error!("Failed to get testebed."))?;
    let json = query.to_json(&*conn)?;

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("GET".into()),
    ))
}

#[endpoint {
    method = POST,
    path = "/v0/testbeds",
    tags = ["testbeds"]
}]
pub async fn api_post_testbed(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonNewTestbed>,
) -> Result<HttpResponseAccepted<()>, HttpError> {
    let db_connection = rqctx.context();

    let json_testbed = body.into_inner();
    let conn = db_connection.lock().await;
    let insert_testbed = InsertTestbed::from_json(json_testbed);
    diesel::insert_into(schema::testbed::table)
        .values(&insert_testbed)
        .execute(&*conn)
        .map_err(|_| http_error!("Failed to save testebed."))?;

    Ok(HttpResponseAccepted(()))
}
