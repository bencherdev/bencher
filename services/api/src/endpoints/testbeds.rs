use std::sync::Arc;

use bencher_json::{
    JsonNewTestbed,
    JsonTestbed,
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
            project::QueryProject,
            testbed::{
                InsertTestbed,
                QueryTestbed,
            },
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
    path =  "/v0/projects/{project}/testbeds",
    tags = ["projects", "testbeds"]
}]
pub async fn dir_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetLsParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/testbeds",
    tags = ["projects", "testbeds"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetLsParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Vec<JsonTestbed>>, CorsHeaders>, HttpError> {
    let api_context = rqctx.context();
    let path_params = path_params.into_inner();

    let api_context = &mut *api_context.lock().await;
    let conn = &mut api_context.db;
    let query_project = QueryProject::from_resource_id(conn, &path_params.project)?;
    let json: Vec<JsonTestbed> = schema::testbed::table
        .filter(schema::testbed::project_id.eq(&query_project.id))
        .order(schema::testbed::name)
        .load::<QueryTestbed>(conn)
        .map_err(|_| http_error!("Failed to get testbeds."))?
        .into_iter()
        .filter_map(|query| query.to_json(conn).ok())
        .collect();

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("GET".into()),
    ))
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/testbeds",
    tags = ["testbeds"]
}]
pub async fn post_options(
    _rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path = "/v0/testbeds",
    tags = ["testbeds"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonNewTestbed>,
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonTestbed>, CorsHeaders>, HttpError> {
    let api_context = rqctx.context();
    let json_testbed = body.into_inner();

    let api_context = &mut *api_context.lock().await;
    let conn = &mut api_context.db;
    let insert_testbed = InsertTestbed::from_json(conn, json_testbed)?;
    diesel::insert_into(schema::testbed::table)
        .values(&insert_testbed)
        .execute(conn)
        .map_err(|_| http_error!("Failed to create testbed."))?;

    let query_testbed = schema::testbed::table
        .filter(schema::testbed::uuid.eq(&insert_testbed.uuid))
        .first::<QueryTestbed>(conn)
        .map_err(|_| http_error!("Failed to create testbed."))?;
    let json = query_testbed.to_json(conn)?;

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(json),
        CorsHeaders::new_auth("POST".into()),
    ))
}

#[derive(Deserialize, JsonSchema)]
pub struct GetOneParams {
    pub project: ResourceId,
    pub testbed: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/testbeds/{testbed}",
    tags = ["projects", "testbeds"]
}]
pub async fn one_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetOneParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/testbeds/{testbed}",
    tags = ["projects", "testbeds"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetOneParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<JsonTestbed>, CorsHeaders>, HttpError> {
    let api_context = rqctx.context();
    let path_params = path_params.into_inner();
    let resource_id = path_params.testbed.as_str();

    let api_context = &mut *api_context.lock().await;
    let conn = &mut api_context.db;
    let project = QueryProject::from_resource_id(conn, &path_params.project)?;
    let query = if let Ok(query) = schema::testbed::table
        .filter(
            schema::testbed::project_id.eq(project.id).and(
                schema::testbed::slug
                    .eq(resource_id)
                    .or(schema::testbed::uuid.eq(resource_id)),
            ),
        )
        .first::<QueryTestbed>(conn)
    {
        Ok(query)
    } else {
        Err(http_error!("Failed to get testbed."))
    }?;
    let json = query.to_json(conn)?;

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("GET".into()),
    ))
}
