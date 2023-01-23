use std::sync::Arc;

use bencher_json::{JsonBackup, JsonEmpty, JsonRestart};
use diesel::connection::SimpleConnection;
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use tracing::{error, warn};

use crate::{
    context::Context,
    endpoints::{
        endpoint::{response_accepted, ResponseAccepted},
        Endpoint, Method,
    },
    error::api_error,
    model::user::auth::AuthUser,
    util::cors::{get_cors, CorsResponse},
    ApiError,
};

use super::Resource;

const BACKUP_RESOURCE: Resource = Resource::Backup;

pub const DEFAULT_DELAY: u64 = 3;

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/backup",
    tags = ["server"]
}]
pub async fn options(
    _rqctx: Arc<RequestContext<Context>>,
    _body: TypedBody<JsonRestart>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<Context>())
}

#[endpoint {
    method = POST,
    path =  "/v0/server/backup",
    tags = ["server"]
}]
pub async fn post(
    rqctx: Arc<RequestContext<Context>>,
    body: TypedBody<JsonBackup>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(BACKUP_RESOURCE, Method::Post);

    let context = rqctx.context();
    let json_restart = body.into_inner();
    let json = post_inner(context, json_restart, &auth_user)
        .await
        .map_err(|e| endpoint.err(e))?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    context: &Context,
    json_backup: JsonBackup,
    auth_user: &AuthUser,
) -> Result<JsonEmpty, ApiError> {
    let api_context = &mut *context.lock().await;
    if !auth_user.is_admin(&api_context.rbac) {
        return Err(ApiError::Admin(auth_user.id));
    }
    let conn = &mut api_context.database;

    let file_name = api_context
        .database_path
        .file_name()
        .unwrap()
        .to_string_lossy();
    warn!("FILENAME: {file_name}");
    let backup_file_name = format!("backup-{file_name}");
    warn!("BACKUP FILENAME: {backup_file_name}");

    let query = if json_backup.vacuum.unwrap_or(true) {
        // sqlite3 /path/to/db "VACUUM INTO '/path/to/backup'"
        format!("VACUUM INTO '{backup_file_name}'")
    } else {
        // sqlite3 /path/to/db '.backup /path/to/backup'
        format!(".backup {backup_file_name}")
    };

    conn.batch_execute(&query).map_err(api_error!())?;

    Ok(JsonEmpty {})
}
