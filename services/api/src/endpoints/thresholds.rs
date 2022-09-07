use std::sync::Arc;

use bencher_json::{
    threshold::{JsonNewThreshold, JsonThreshold},
    ResourceId,
};
use diesel::{expression_methods::BoolExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl};
use dropshot::{
    endpoint, HttpError, HttpResponseAccepted, HttpResponseHeaders, HttpResponseOk, Path,
    RequestContext, TypedBody,
};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    db::{
        model::{
            branch::QueryBranch,
            project::QueryProject,
            testbed::QueryTestbed,
            threshold::{InsertThreshold, QueryThreshold},
            user::QueryUser,
        },
        schema,
    },
    diesel::ExpressionMethods,
    util::{cors::get_cors, headers::CorsHeaders, http_error, Context},
};

#[derive(Deserialize, JsonSchema)]
pub struct GetLsParams {
    pub project: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/thresholds",
    tags = ["projects", "thresholds"]
}]
pub async fn dir_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetLsParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/thresholds",
    tags = ["projects", "thresholds"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetLsParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Vec<JsonThreshold>>, CorsHeaders>, HttpError> {
    let user_id = QueryUser::auth(&rqctx).await?;
    let path_params = path_params.into_inner();
    let project_id = QueryProject::connection(&rqctx, user_id, &path_params.project).await?;

    let context = &mut *rqctx.context().lock().await;
    let conn = &mut context.db;
    let json: Vec<JsonThreshold> = schema::threshold::table
        .left_join(schema::testbed::table.on(schema::threshold::testbed_id.eq(schema::testbed::id)))
        .filter(schema::testbed::project_id.eq(project_id))
        .order(schema::threshold::id)
        .select((
            schema::threshold::id,
            schema::threshold::uuid,
            schema::threshold::branch_id,
            schema::threshold::testbed_id,
            schema::threshold::kind,
            schema::threshold::statistic_id,
        ))
        .order(schema::threshold::id)
        .load::<QueryThreshold>(conn)
        .map_err(|_| http_error!("Failed to get threshold."))?
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
    path =  "/v0/thresholds",
    tags = ["thresholds"]
}]
pub async fn post_options(
    _rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path = "/v0/thresholds",
    tags = ["thresholds"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonNewThreshold>,
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonThreshold>, CorsHeaders>, HttpError> {
    QueryUser::auth(&rqctx).await?;

    const ERROR: &str = "Failed to create thresholds.";

    let json_threshold = body.into_inner();

    let context = &mut *rqctx.context().lock().await;
    let conn = &mut context.db;

    let branch_id = QueryBranch::get_id(conn, &json_threshold.branch)?;
    let testbed_id = QueryTestbed::get_id(conn, &json_threshold.testbed)?;
    let branch_project_id = schema::branch::table
        .filter(schema::branch::id.eq(&branch_id))
        .select(schema::branch::project_id)
        .first::<i32>(conn)
        .map_err(|_| http_error!(ERROR))?;
    let testbed_project_id = schema::testbed::table
        .filter(schema::testbed::id.eq(&testbed_id))
        .select(schema::testbed::project_id)
        .first::<i32>(conn)
        .map_err(|_| http_error!(ERROR))?;
    if branch_project_id != testbed_project_id {
        return Err(http_error!(ERROR));
    }

    let insert_threshold = InsertThreshold::from_json(conn, json_threshold)?;
    diesel::insert_into(schema::threshold::table)
        .values(&insert_threshold)
        .execute(conn)
        .map_err(|_| http_error!(ERROR))?;

    let query_threshold = schema::threshold::table
        .filter(schema::threshold::uuid.eq(&insert_threshold.uuid))
        .first::<QueryThreshold>(conn)
        .map_err(|_| http_error!(ERROR))?;
    let json = query_threshold.to_json(conn)?;

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(json),
        CorsHeaders::new_auth("POST".into()),
    ))
}

#[derive(Deserialize, JsonSchema)]
pub struct GetOneParams {
    pub project: ResourceId,
    pub threshold: Uuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}/thresholds/{threshold}",
    tags = ["projects", "thresholds"]
}]
pub async fn one_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetOneParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/projects/{project}/thresholds/{threshold}",
    tags = ["projects", "thresholds"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetOneParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<JsonThreshold>, CorsHeaders>, HttpError> {
    let user_id = QueryUser::auth(&rqctx).await?;
    let path_params = path_params.into_inner();
    let project_id = QueryProject::connection(&rqctx, user_id, &path_params.project).await?;
    let threshold_uuid = path_params.threshold.to_string();

    let context = &mut *rqctx.context().lock().await;
    let conn = &mut context.db;
    let query = if let Ok(query) = schema::threshold::table
        .left_join(schema::testbed::table.on(schema::threshold::testbed_id.eq(schema::testbed::id)))
        .filter(
            schema::testbed::project_id
                .eq(project_id)
                .and(schema::threshold::uuid.eq(&threshold_uuid)),
        )
        .select((
            schema::threshold::id,
            schema::threshold::uuid,
            schema::threshold::branch_id,
            schema::threshold::testbed_id,
            schema::threshold::kind,
            schema::threshold::statistic_id,
        ))
        .first::<QueryThreshold>(conn)
    {
        Ok(query)
    } else {
        Err(http_error!("Failed to get threshold."))
    }?;
    let json = query.to_json(conn)?;

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("GET".into()),
    ))
}
