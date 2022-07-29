use std::{
    str::FromStr,
    sync::Arc,
};

use bencher_json::{
    JsonNewProject,
    JsonNewReport,
    JsonProject,
};
use chrono::NaiveDateTime;
use diesel::{
    QueryDsl,
    RunQueryDsl,
    SqliteConnection,
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
        model::{
            adapter::QueryAdapter,
            project::{
                InsertProject,
                QueryProject,
            },
            report::{
                InsertReport,
                QueryReport,
            },
            testbed::QueryTestbed,
            user::QueryUser,
        },
        schema,
    },
    diesel::ExpressionMethods,
    util::{
        auth::get_token,
        headers::CorsHeaders,
        http_error,
        Context,
    },
};

#[endpoint {
    method = GET,
    path = "/v0/projects",
    tags = ["projects"]
}]
pub async fn api_get_projects(
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

#[derive(Deserialize, JsonSchema)]
pub struct PathParams {
    pub project: String,
}

#[endpoint {
    method = GET,
    path = "/v0/projects/{project}",
    tags = ["projects"]
}]
pub async fn api_get_project_slug(
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

#[endpoint {
    method = POST,
    path = "/v0/projects",
    tags = ["projects"]
}]
pub async fn api_post_project(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonNewProject>,
) -> Result<HttpResponseAccepted<()>, HttpError> {
    let uuid = get_token(&rqctx).await?;
    let db_connection = rqctx.context();
    let json_project = body.into_inner();
    let conn = db_connection.lock().await;
    let insert_project = InsertProject::from_json(&*conn, &uuid, json_project)?;
    diesel::insert_into(schema::project::table)
        .values(&insert_project)
        .execute(&*conn)
        .map_err(|_| http_error!("Failed to create project."))?;

    Ok(HttpResponseAccepted(()))
}
