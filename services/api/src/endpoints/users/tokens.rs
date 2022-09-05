use std::sync::Arc;

use bencher_json::{JsonAuthToken, JsonNewToken, JsonToken, ResourceId};
use diesel::{expression_methods::BoolExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::{
    endpoint, HttpError, HttpResponseAccepted, HttpResponseHeaders, HttpResponseOk, Path,
    RequestContext, TypedBody,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::{
    db::{
        model::{
            user::token::{InsertToken, QueryToken},
            user::QueryUser,
        },
        schema,
    },
    diesel::ExpressionMethods,
    util::{cors::get_cors, headers::CorsHeaders, http_error, Context},
};

#[derive(Deserialize, JsonSchema)]
pub struct GetLsParams {
    pub user: ResourceId,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/users/{user}/tokens",
    tags = ["users", "tokens"]
}]
pub async fn dir_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetLsParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/users/{user}/tokens",
    tags = ["users", "tokens"]
}]
pub async fn get_ls(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetLsParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<Vec<JsonToken>>, CorsHeaders>, HttpError> {
    let user_id = QueryUser::auth(&rqctx).await?;
    let path_params = path_params.into_inner();

    let context = &mut *rqctx.context().lock().await;
    let conn = &mut context.db;
    let json: Vec<JsonToken> = schema::token::table
        .filter(schema::token::user_id.eq(user_id))
        .order((schema::token::creation, schema::token::expiration))
        .load::<QueryToken>(conn)
        .map_err(|_| http_error!("Failed to get tokens."))?
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
    path =  "/v0/tokens",
    tags = ["tokens"]
}]
pub async fn post_options(
    _rqctx: Arc<RequestContext<Context>>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path = "/v0/tokens",
    tags = ["tokens"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonNewToken>,
) -> Result<HttpResponseHeaders<HttpResponseAccepted<JsonAuthToken>, CorsHeaders>, HttpError> {
    QueryUser::auth(&rqctx).await?;
    let json_branch = body.into_inner();

    let context = &mut *rqctx.context().lock().await;
    let conn = &mut context.db;
    let insert_branch = InsertToken::from_json(conn, json_branch)?;
    diesel::insert_into(schema::token::table)
        .values(&insert_branch)
        .execute(conn)
        .map_err(|_| http_error!("Failed to create token."))?;

    let query_branch = schema::token::table
        .filter(schema::token::uuid.eq(&insert_branch.uuid))
        .first::<QueryToken>(conn)
        .map_err(|_| http_error!("Failed to create token."))?;
    let json = query_branch.to_json(conn)?;

    Ok(HttpResponseHeaders::new(
        HttpResponseAccepted(json),
        CorsHeaders::new_auth("POST".into()),
    ))
}

#[derive(Deserialize, JsonSchema)]
pub struct GetOneParams {
    pub user: ResourceId,
    pub token: Uuid,
}

#[endpoint {
    method = OPTIONS,
    path =  "/v0/users/{user}/tokens/{token}",
    tags = ["users", "tokens"]
}]
pub async fn one_options(
    _rqctx: Arc<RequestContext<Context>>,
    _path_params: Path<GetOneParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<String>>, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = GET,
    path =  "/v0/users/{user}/tokens/{token}",
    tags = ["users", "tokens"]
}]
pub async fn get_one(
    rqctx: Arc<RequestContext<Context>>,
    path_params: Path<GetOneParams>,
) -> Result<HttpResponseHeaders<HttpResponseOk<JsonAuthToken>, CorsHeaders>, HttpError> {
    let user_id = QueryUser::auth(&rqctx).await?;
    let path_params = path_params.into_inner();
    let project_id = QueryProject::connection(&rqctx, user_id, &path_params.user).await?;
    let resource_id = path_params.token.as_str();

    let context = &mut *rqctx.context().lock().await;
    let conn = &mut context.db;
    let query = if let Ok(query) = schema::token::table
        .filter(
            schema::token::project_id.eq(project_id).and(
                schema::token::slug
                    .eq(resource_id)
                    .or(schema::token::uuid.eq(resource_id)),
            ),
        )
        .first::<QueryToken>(conn)
    {
        Ok(query)
    } else {
        Err(http_error!("Failed to get token."))
    }?;
    let json = query.to_json(conn)?;

    Ok(HttpResponseHeaders::new(
        HttpResponseOk(json),
        CorsHeaders::new_pub("GET".into()),
    ))
}
