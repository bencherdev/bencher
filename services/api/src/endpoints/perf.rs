use std::sync::Arc;

use bencher_json::{
    JsonBranch,
    JsonNewBranch,
    ResourceId,
};
use diesel::{
    expression_methods::BoolExpressionMethods,
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
use serde::Deserialize;

use crate::{
    db::{
        model::{
            branch::{
                InsertBranch,
                QueryBranch,
            },
            project::QueryProject,
        },
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

#[derive(Deserialize, JsonSchema)]
pub struct GetLsParams {
    pub project: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/perf",
    tags = ["projects", "perf"]
}]
pub async fn options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetLsParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/perf",
    tags = ["projects", "perf"]
}]
pub async fn get(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetLsParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Vec<JsonBranch>>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();
    let path_params = path_params.into_inner();

    let conn = db_connection.lock().await;
    let query_project = QueryProject::from_resource_id(&*conn, &path_params.project)?;
    let json: Vec<JsonBranch> = schema::branch::table
        .filter(schema::branch::project_id.eq(&query_project.id))
        .order(schema::branch::name)
        .load::<QueryBranch>(&*conn)
        .map_err(|_| http_error!("Failed to get branches."))?
        .into_iter()
        .filter_map(|query| query.to_json(&*conn).ok())
        .collect();

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("GET".into()),
    ))
}
