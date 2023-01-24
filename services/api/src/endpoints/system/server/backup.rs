use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::{ffi::OsStr, io::prelude::*};

use bencher_json::system::backup::JsonDataStore;
use bencher_json::{JsonBackup, JsonEmpty, JsonRestart};
use chrono::Utc;
use diesel::connection::SimpleConnection;
use dropshot::{endpoint, HttpError, RequestContext, TypedBody};
use flate2::{Compression, GzBuilder};
use tokio::fs::remove_file;
use tokio::io::AsyncReadExt;
use tracing::warn;

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
    let conn = &mut api_context.database.connection;

    // Create a database backup
    let mut backup_file_path = api_context.database.path.clone();
    let file_stem = backup_file_path
        .file_stem()
        .unwrap_or_else(|| OsStr::new("bencher"))
        .to_string_lossy();
    let file_extension = backup_file_path
        .extension()
        .unwrap_or_else(|| OsStr::new("db"))
        .to_string_lossy();
    let date_time = Utc::now();
    let backup_file_name = format!(
        "backup-{file_stem}-{}.{file_extension}",
        date_time.format("%Y-%m-%d-%H-%M-%S")
    );
    backup_file_path.set_file_name(&backup_file_name);
    let backup_file_path_str = backup_file_path.to_string_lossy();
    let query = format!("VACUUM INTO '{backup_file_path_str}'");

    conn.batch_execute(&query).map_err(api_error!())?;

    // Compress the database backup
    let db_file_path = if json_backup.compress.unwrap_or_default() {
        let compress_file_name = format!("{backup_file_name}.gz");
        let mut compress_file_path = backup_file_path.clone();
        compress_file_path.set_file_name(&compress_file_name);

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
            .filename(api_context.database.path.file_name().unwrap().as_bytes())
            .comment("Bencher database backup")
            .write(compress_file, Compression::default());
        gz.write_all(&backup_contents)
            .map_err(ApiError::BackupFile)?;
        gz.finish().map_err(ApiError::BackupFile)?;

        remove_file(backup_file_path).await.unwrap();

        compress_file_path
    } else {
        backup_file_path.clone()
    };

    if let Some(JsonDataStore::AwsS3) = json_backup.data_store {
        let access_key_id = std::env::var("AWS_ACCESS_KEY_ID").unwrap();
        let secret_access_key = std::env::var("AWS_SECRET_ACCESS_KEY").unwrap();
        let credentials =
            aws_sdk_s3::Credentials::new(access_key_id, secret_access_key, None, None, "bencher");
        let credentials_provider =
            aws_credential_types::provider::SharedCredentialsProvider::new(credentials);

        const AWS_S3_ARN: &str = "arn:aws:s3:";
        const COLON: &str = ":";
        const ACCESSPOINT: &str = ":accesspoint/";
        let aws_bucket = std::env::var("AWS_BUCKET").unwrap();
        let (region, arn) = aws_bucket
            .trim_start_matches(AWS_S3_ARN)
            .split_once(COLON)
            .unwrap();
        warn!("REGION: {region}");
        let (id, resource) = arn.split_once(ACCESSPOINT).unwrap();
        let (bucket_name, key) = if let Some((bucket, key_path)) = resource.split_once("/") {
            let key_path = Path::new(key_path);
            let key_path = key_path.join(db_file_path.file_name().unwrap());
            (bucket.to_string(), key_path.to_string_lossy().to_string())
        } else {
            (
                resource.to_string(),
                db_file_path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string(),
            )
        };
        let bucket = format!("{AWS_S3_ARN}{region}{COLON}{id}{ACCESSPOINT}{bucket_name}");
        warn!("BUCKET: {bucket}");

        let config = aws_sdk_s3::Config::builder()
            .credentials_provider(credentials_provider)
            .region(aws_sdk_s3::Region::new(region.to_string()))
            .build();

        let client = aws_sdk_s3::Client::from_conf(config);

        // let endpoint_url = std::env::var("LITESTREAM_REPLICA_URL").unwrap();
        // let s3_uri = http::uri::Uri::from_str(&endpoint_url).unwrap();
        // let scheme = s3_uri.scheme_str().unwrap();
        // warn!("SCHEME {scheme}");
        // let bucket = s3_uri.host().unwrap();
        // warn!("BUCKET {bucket}");
        // let key = s3_uri.path();
        // warn!("KEY {key}");
        let body = aws_sdk_s3::types::ByteStream::from_path(&db_file_path)
            .await
            .unwrap();
        client
            .put_object()
            .bucket(bucket)
            .key(key)
            .body(body)
            .send()
            .await
            .unwrap();
    }

    if json_backup.rm.unwrap_or_default() {
        remove_file(db_file_path).await.unwrap();
    }

    Ok(JsonEmpty {})
}
