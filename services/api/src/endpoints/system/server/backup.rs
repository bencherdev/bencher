use std::{ffi::OsStr, path::PathBuf};

use async_compression::tokio::write::GzipEncoder;
use bencher_json::system::backup::JsonDataStore;
use bencher_json::{JsonBackup, JsonEmpty, JsonRestart};
use chrono::Utc;
use diesel::connection::SimpleConnection;
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use tokio::fs::remove_file;
use tokio::io::{AsyncReadExt, BufWriter};
use tokio::io::{AsyncWriteExt, BufReader};

use crate::{
    context::ApiContext,
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
const BUFFER_SIZE: usize = 1024;

#[allow(clippy::unused_async)]
#[endpoint {
    method = OPTIONS,
    path =  "/v0/server/backup",
    tags = ["server"]
}]
pub async fn server_backup_options(
    _rqctx: RequestContext<ApiContext>,
    _body: TypedBody<JsonRestart>,
) -> Result<CorsResponse, HttpError> {
    Ok(get_cors::<ApiContext>())
}

#[endpoint {
    method = POST,
    path =  "/v0/server/backup",
    tags = ["server"]
}]
pub async fn server_backup_post(
    rqctx: RequestContext<ApiContext>,
    body: TypedBody<JsonBackup>,
) -> Result<ResponseAccepted<JsonEmpty>, HttpError> {
    let auth_user = AuthUser::new(&rqctx).await?;
    let endpoint = Endpoint::new(BACKUP_RESOURCE, Method::Post);

    let context = rqctx.context();
    let json_restart = body.into_inner();
    let json = post_inner(context, json_restart, &auth_user)
        .await
        .map_err(|e| {
            if let ApiError::HttpError(e) = e {
                e
            } else {
                endpoint.err(e).into()
            }
        })?;

    response_accepted!(endpoint, json)
}

async fn post_inner(
    context: &ApiContext,
    json_backup: JsonBackup,
    auth_user: &AuthUser,
) -> Result<JsonEmpty, ApiError> {
    if !auth_user.is_admin(&context.rbac) {
        return Err(ApiError::Admin(auth_user.id));
    }

    // Create a database backup
    let (backup_file_path, backup_file_name) = backup_database(context).await?;

    // Compress the database backup
    let (source_path, file_name) = if json_backup.compress.unwrap_or_default() {
        compress_database(backup_file_path.clone(), &backup_file_name).await?
    } else {
        (backup_file_path.clone(), backup_file_name)
    };

    // Store the database backup in AWS S3
    if let Some(JsonDataStore::AwsS3) = json_backup.data_store {
        if let Some(data_store) = &context.database.data_store {
            data_store.backup(&source_path, &file_name).await?;
        } else {
            return Err(ApiError::AwsS3("No data store".into()));
        };
    }

    // Remove the remaining database backup
    if json_backup.rm.unwrap_or_default() {
        remove_file(source_path)
            .await
            .map_err(ApiError::BackupFile)?;
    }

    Ok(JsonEmpty {})
}

async fn backup_database(context: &ApiContext) -> Result<(PathBuf, String), ApiError> {
    let conn = &mut *context.conn().await;
    let mut file_path = context.database.path.clone();

    let file_stem = file_path
        .file_stem()
        .unwrap_or_else(|| OsStr::new("bencher"))
        .to_string_lossy();
    let file_extension = file_path
        .extension()
        .unwrap_or_else(|| OsStr::new("db"))
        .to_string_lossy();
    let date_time = Utc::now();
    let file_name = format!(
        "backup-{file_stem}-{}.{file_extension}",
        date_time.format("%Y-%m-%d-%H-%M-%S")
    );
    file_path.set_file_name(&file_name);
    let file_path_str = file_path.to_string_lossy();
    let query = format!("VACUUM INTO '{file_path_str}'");

    conn.batch_execute(&query).map_err(api_error!())?;

    Ok((file_path, file_name))
}

async fn compress_database(
    backup_file_path: PathBuf,
    backup_file_name: &str,
) -> Result<(PathBuf, String), ApiError> {
    let backup_file = tokio::fs::File::open(&backup_file_path)
        .await
        .map_err(ApiError::BackupFile)?;
    let mut backup_data = BufReader::with_capacity(BUFFER_SIZE, backup_file);

    let compress_file_name = format!("{backup_file_name}.gz");
    let mut compress_file_path = backup_file_path.clone();
    compress_file_path.set_file_name(&compress_file_name);
    let compress_file = tokio::fs::File::create(&compress_file_path)
        .await
        .map_err(ApiError::BackupFile)?;
    let compress_data = BufWriter::with_capacity(BUFFER_SIZE, compress_file);

    let mut encoder = GzipEncoder::new(compress_data);
    let mut data_buffer = [0; BUFFER_SIZE];
    while let Ok(data_size) = backup_data.read(&mut data_buffer).await {
        if data_size == 0 {
            break;
        }

        encoder
            .write_all(&data_buffer)
            .await
            .map_err(ApiError::BackupFile)?;
    }
    encoder.shutdown().await.map_err(ApiError::BackupFile)?;

    remove_file(backup_file_path)
        .await
        .map_err(ApiError::BackupFile)?;

    Ok((compress_file_path, compress_file_name))
}
