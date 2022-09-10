use std::sync::Arc;

use bencher_json::{JsonNewProject, JsonProject, ResourceId};
use bencher_rbac::project::Role;
use diesel::{BoolExpressionMethods, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{
    endpoint, HttpError, HttpResponseAccepted, HttpResponseHeaders, HttpResponseOk, Path,
    RequestContext, TypedBody,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    db::model::{
        project::{InsertProject, QueryProject},
        user::{project::InsertProjectRole, QueryUser},
    },
    schema,
    util::{cors::get_cors, headers::CorsHeaders, http_error, Context},
};

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects",
    tags = ["projects"]
}]
pub async fn dir_options(
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
    QueryUser::auth(&rqctx).await?;

    let context = &mut *rqctx.context().lock().await;
    let conn = &mut context.db;
    let json: Vec<JsonProject> = schema::project::table
        // TODO actually filter here with `bencher_rbac`
        // .filter(schema::project::owner_id.eq(user_id))
        .order(schema::project::name)
        .load::<QueryProject>(conn)
        .map_err(|_| http_error!("Failed to get projects."))?
        .into_iter()
        .filter_map(|query| query.to_json(conn).ok())
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
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonProject>, CorsHeaders>, HttpError> {
    let user_id = QueryUser::auth(&rqctx).await?;

    let json_project = body.into_inner();

    let context = &mut *rqctx.context().lock().await;
    let conn = &mut context.db;

    // Create the project
    let insert_project = InsertProject::from_json(conn, json_project)?;
    diesel::insert_into(schema::project::table)
        .values(&insert_project)
        .execute(conn)
        .map_err(|_| http_error!("Failed to create project."))?;
    let query_project = schema::project::table
        .filter(schema::project::uuid.eq(&insert_project.uuid))
        .first::<QueryProject>(conn)
        .map_err(|_| http_error!("Failed to create project."))?;

    // Connect the user to the project as a `Maintainer`
    let insert_proj_role = InsertProjectRole {
        user_id,
        project_id: query_project.id,
        role: Role::Maintainer.to_string(),
    };
    diesel::insert_into(schema::project_role::table)
        .values(&insert_proj_role)
        .execute(conn)
        .map_err(|_| http_error!("Failed to create project."))?;

    let json = query_project.to_json(conn)?;

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(json),
        CorsHeaders::new_auth("POST".into()),
    ))
}

#[derive(Deserialize, JsonSchema)]
pub struct PathParams {
    pub project: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/projects/{project}",
    tags = ["projects"]
}]
pub async fn one_options(
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
    let user_id = QueryUser::auth(&rqctx).await?;
    let path_params = path_params.into_inner();

    let context = &mut *rqctx.context().lock().await;
    let conn = &mut context.db;

    let project = &path_params.project.0;
    let query = schema::project::table
        .filter(
            schema::project::slug
                .eq(project)
                .or(schema::project::uuid.eq(project)),
        )
        .first::<QueryProject>(conn)
        .map_err(|_| http_error!("Failed to get project."))?;

    QueryUser::has_access(conn, user_id, query.id)?;
    let json = query.to_json(conn)?;

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("GET".into()),
    ))
}
