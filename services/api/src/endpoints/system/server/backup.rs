use std::sync::Arc;
use std::{ffi::OsStr, io::prelude::*};

use bencher_json::{JsonBackup, JsonEmpty, JsonRestart};
use diesel::connection::SimpleConnection;
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use flate2::{Compression, GzBuilder};
use tokio::io::AsyncReadExt;

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
const GZIP_EXTENSION: &str = "gz";

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

    // Create a database backup
    let file_name = api_context
        .database_path
        .file_name()
        .unwrap_or_else(|| OsStr::new("bencher.db"))
        .to_string_lossy();
    let backup_file_name = format!("backup-{file_name}");
    let mut backup_file_path = api_context.database_path.clone();
    backup_file_path.set_file_name(&backup_file_name);
    let backup_file_path_str = backup_file_path.to_string_lossy();
    let query = format!("VACUUM INTO '{backup_file_path_str}'");

    conn.batch_execute(&query).map_err(api_error!())?;

    // Compress the database backup
    if json_backup.compress.unwrap_or_default() {
        let mut compress_file_path = backup_file_path.clone();
        let compress_extension = if let Some(extension) = compress_file_path.extension() {
            format!("{}.{GZIP_EXTENSION}", extension.to_string_lossy())
        } else {
            GZIP_EXTENSION.into()
        };
        compress_file_path.set_extension(compress_extension);

        let mut backup_file = tokio::fs::File::open(&backup_file_path)
            .await
            .map_err(ApiError::BackupFile)?;
        let mut backup_contents = Vec::new();
        backup_file
            .read_to_end(&mut backup_contents)
            .await
            .map_err(ApiError::BackupFile)?;

        let compress_file =
            std::fs::File::create(&compress_file_path).map_err(ApiError::BackupFile)?;
        let mut gz = GzBuilder::new()
            .filename(file_name.as_bytes())
            .comment("Bencher database backup")
            .write(compress_file, Compression::default());
        gz.write_all(&backup_contents)
            .map_err(ApiError::BackupFile)?;
        gz.finish().map_err(ApiError::BackupFile)?;
    }

    Ok(JsonEmpty {})
}
