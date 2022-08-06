use std::sync::Arc;

use bencher_json::JsonAdapter;
use diesel::{
    QueryDsl,
    RunQueryDsl,
};
use dropshot::{
    endpoint,
    HttpError,
    HttpResponseHeaders,
    HttpResponseOk,
    Path,
    RequestContext,
};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    db::{
        model::adapter::QueryAdapter,
        schema,
    },
    diesel::ExpressionMethods,
    util::{
        cors::get_cors,
        headers::CorsHeaders,
        http_error,
        Context,
    },
};

#[endpoint {
    method = OPTIONS,
    path =  "/v0/adapters",
    tags = ["projects", "branches"]
}]
pub async fn get_ls_options(
    _rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path = "/v0/adapters",
    tags = ["adapters"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Vec<JsonAdapter>>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();

    let conn = db_connection.lock().await;
    let json: Vec<JsonAdapter> = schema::adapter::table
        .load::<QueryAdapter>(&*conn)
        .map_err(|_| http_error!("Failed to get adapters."))?
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
    pub adapter_uuid: Uuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/adapters/{adapter_uuid}",
    tags = ["projects"]
}]
pub async fn get_one_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<PathParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path = "/v0/adapters/{adapter_uuid}",
    tags = ["adapters"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<PathParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<JsonAdapter>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();
    let path_params = path_params.into_inner();

    let conn = db_connection.lock().await;
    let query = schema::adapter::table
        .filter(schema::adapter::uuid.eq(&path_params.adapter_uuid.to_string()))
        .first::<QueryAdapter>(&*conn)
        .map_err(|_| http_error!("Failed to get adapter."))?;
    let json = query.to_json(&*conn)?;

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("GET".into()),
    ))
}
