use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseCreated};
use bencher_json::{JsonBackup, JsonBackupCreated, JsonRestart};
use bencher_schema::{
    context::ApiContext,
    error::bad_request_error,
    model::{
        server::ServerBackup,
        user::{admin::AdminUser, auth::BearerToken},
    },
};
use dropshot::{HttpError, RequestContext, TypedBody, endpoint};

#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/backup",
    tags = ["server"]
}]
pub async fn server_backup_options(
    _rqctx: RequestContext<ApiContext>,
    _body: TypedBody<JsonRestart>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Backup server
///
/// Backup the API server database.
/// The user must be an admin on the server to use this route.
#[endpoint {
    method = POST,
    path =  "/v0/server/backup",
    tags = ["server"]
}]
pub async fn server_backup_post(
    rqctx: RequestContext<ApiContext>,
    bearer_token: BearerToken,
    body: TypedBody<JsonBackup>,
) -> Result<ResponseCreated<JsonBackupCreated>, HttpError> {
    let _admin_user = AdminUser::from_token(rqctx.context(), bearer_token).await?;
    let json = post_inner(rqctx.context(), body.into_inner()).await?;
    Ok(Post::auth_response_created(json))
}

async fn post_inner(
    context: &ApiContext,
    json_backup: JsonBackup,
) -> Result<JsonBackupCreated, HttpError> {
    ServerBackup::run(
        context.database.path.clone(),
        context.database.data_store.as_ref(),
        json_backup,
    )
    .await
    .map_err(bad_request_error)
}
