use std::sync::Arc;

use bencher_json::{
    JsonNewProject,
    JsonProject,
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
    UntypedBody,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    db::{
        model::{
            project::{
                InsertProject,
                QueryProject,
            },
            user::QueryUser,
        },
        schema,
    },
    diesel::ExpressionMethods,
    util::{
        auth::get_token,
        cors::get_cors,
        headers::CorsHeaders,
        http_error,
        Context,
    },
};

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects",
    tags = ["projects"]
}]
pub async fn options(
    _rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path = "/v0/projects",
    tags = ["projects"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Vec<JsonProject>>, CorsHeaders>, HttpError> {
    let uuid = get_token(&rqctx).await?;
    let db_connection = rqctx.context();
    let conn = db_connection.lock().await;
    let owner_id = QueryUser::get_id(&*conn, &uuid)?;
    let json: Vec<JsonProject> = schema::project::table
        .filter(schema::project::owner_id.eq(owner_id))
        .load::<QueryProject>(&*conn)
        .map_err(|_| http_error!("Failed to get projects."))?
        .into_iter()
        .filter_map(|query| query.to_json(&*conn).ok())
        .collect();

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_auth("GET".into()),
    ))
}

#[endpoint {
    method = POST,
    path = "/v0/projects",
    tags = ["projects"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonNewProject>,
) -> Result<HttpResponseHeaders<HttpResponseAccepted<()>, CorsHeaders>, HttpError> {
    let uuid = get_token(&rqctx).await?;
    let db_connection = rqctx.context();
    let json_project = body.into_inner();
    let conn = db_connection.lock().await;
    let insert_project = InsertProject::from_json(&*conn, &uuid, json_project)?;
    diesel::insert_into(schema::project::table)
        .values(&insert_project)
        .execute(&*conn)
        .map_err(|_| http_error!("Failed to create project."))?;

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(()),
        CorsHeaders::new_auth("POST".into()),
    ))
}

#[derive(Deserialize, JsonSchema)]
pub struct PathParams {
    pub project: String,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}",
    tags = ["projects"]
}]
pub async fn options_params(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<PathParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path = "/v0/projects/{project}",
    tags = ["projects"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<PathParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<JsonProject>, CorsHeaders>, HttpError> {
    let db_connection = rqctx.context();
    let path_params = path_params.into_inner();
    let conn = db_connection.lock().await;

    let query = if let Ok(query) = schema::project::table
        .filter(schema::project::slug.eq(&path_params.project))
        .first::<QueryProject>(&*conn)
    {
        query
    } else if let Ok(query) = schema::project::table
        .filter(schema::project::uuid.eq(&path_params.project))
        .first::<QueryProject>(&*conn)
    {
        query
    } else {
        return Err(http_error!("Failed to get project."));
    };
    let json = query.to_json(&*conn)?;

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("GET".into()),
    ))
}
