use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use bencher_json::{Secret, system::config::DataStore as DataStoreConfig};
use camino::Utf8PathBuf;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use dropshot::HttpError;

use crate::error::issue_error;

pub type DbConnection = diesel::SqliteConnection;

pub struct Database {
    pub path: PathBuf,
    pub connection: Arc<tokio::sync::Mutex<DbConnection>>,
    pub public_pool: Pool<ConnectionManager<DbConnection>>,
    pub auth_pool: Pool<ConnectionManager<DbConnection>>,
    pub data_store: Option<DataStore>,
}

impl Database {
    pub async fn get_public_conn(
        &self,
    ) -> Result<PooledConnection<ConnectionManager<DbConnection>>, HttpError> {
        Self::get_conn(self.public_pool.clone()).await
    }

    pub async fn get_auth_conn(
        &self,
    ) -> Result<PooledConnection<ConnectionManager<DbConnection>>, HttpError> {
        if let Some(conn) = self.public_pool.try_get() {
            return Ok(conn);
        }
        Self::get_conn(self.auth_pool.clone()).await
    }

    async fn get_conn(
        pool: Pool<ConnectionManager<DbConnection>>,
    ) -> Result<PooledConnection<ConnectionManager<DbConnection>>, HttpError> {
        tokio::task::spawn_blocking(move || {
            pool.get().map_err(|e| {
                issue_error(
                    "Failed to get database connection from pool",
                    "Failed to get a database connection from pool:",
                    e,
                )
            })
        })
        .await
        .map_err(|e| {
            issue_error(
                "Failed to join database connection task from pool",
                "Failed to join the database connection task from pool:",
                e,
            )
        })?
    }
}

pub enum DataStore {
    AwsS3(AwsS3),
}

pub struct AwsS3 {
    client: aws_sdk_s3::Client,
    s3_arn: S3Arn,
}

#[derive(Debug, thiserror::Error)]
pub enum DataStoreError {
    #[error("Failed to configure data store: {0}")]
    DataStore(String),
    #[error("Failed to use AWS S3: {0}")]
    AwsS3(String),
    #[error("Failed to parse S3 ARN ({access_point}): {error}")]
    S3Arn {
        access_point: String,
        error: S3ArnError,
    },
}

impl TryFrom<DataStoreConfig> for DataStore {
    type Error = DataStoreError;

    fn try_from(data_store: DataStoreConfig) -> Result<Self, Self::Error> {
        match data_store {
            DataStoreConfig::AwsS3 {
                access_key_id,
                secret_access_key,
                access_point,
            } => AwsS3::new(access_key_id, secret_access_key, &access_point).map(Self::AwsS3),
        }
    }
}

impl DataStore {
    pub async fn backup(&self, source_path: &Path, file_name: &str) -> Result<(), DataStoreError> {
        match self {
            Self::AwsS3(aws_s3) => aws_s3.backup(source_path, file_name).await,
        }
    }
}

impl AwsS3 {
    fn new(
        access_key_id: String,
        secret_access_key: Secret,
        access_point: &str,
    ) -> Result<Self, DataStoreError> {
        let credentials = aws_credential_types::Credentials::new(
            access_key_id,
            secret_access_key,
            None,
            None,
            "bencher",
        );
        let credentials_provider =
            aws_credential_types::provider::SharedCredentialsProvider::new(credentials);
        let s3_arn: S3Arn = access_point
            .parse()
            .map_err(|error| DataStoreError::S3Arn {
                access_point: access_point.to_owned(),
                error,
            })?;

        let config = aws_sdk_s3::Config::builder()
            .credentials_provider(credentials_provider)
            .region(aws_sdk_s3::config::Region::new(s3_arn.region.clone()))
            .build();
        let client = aws_sdk_s3::Client::from_conf(config);

        Ok(Self { client, s3_arn })
    }

    async fn backup(&self, source_path: &Path, file_name: &str) -> Result<(), DataStoreError> {
        let key = if let Some(bucket_path) = &self.s3_arn.bucket_path {
            Utf8PathBuf::from(bucket_path).join(file_name).to_string()
        } else {
            file_name.to_owned()
        };

        let body = aws_sdk_s3::primitives::ByteStream::from_path(source_path)
            .await
            .map_err(|e| DataStoreError::AwsS3(e.to_string()))?;

        self.client
            .put_object()
            .bucket(self.s3_arn.bucket_arn())
            .key(key)
            .body(body)
            .send()
            .await
            .map_err(|e| DataStoreError::AwsS3(e.to_string()))?;

        Ok(())
    }
}

// https://docs.aws.amazon.com/IAM/latest/UserGuide/reference-arns.html
struct S3Arn {
    partition: String,
    region: String,
    account_id: String,
    bucket_name: String,
    bucket_path: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum S3ArnError {
    #[error("Missing S3 ARN prefix")]
    NoPrefix,
    #[error("Invalid S3 ARN prefix: {0}")]
    BadPrefix(String),
    #[error("Missing S3 ARN partition")]
    NoPartition,
    #[error("Missing S3 ARN service")]
    NoService,
    #[error("Invalid S3 ARN service: {0}")]
    BadService(String),
    #[error("Missing S3 ARN region")]
    NoRegion,
    #[error("Missing S3 ARN account ID")]
    NoAccountId,
    #[error("Missing S3 ARN resource")]
    NoResource,
    #[error("Unexpected extra data in S3 ARN: {0:?}")]
    Remainder(Vec<String>),
    #[error("Missing S3 ARN access point")]
    NoAccessPoint,
    #[error("Invalid S3 ARN access point: {0}")]
    BadAccessPoint(String),
    #[error("Missing S3 ARN bucket name")]
    NoBucketName,
    #[error("Invalid S3 ARN bucket name: {0}")]
    BadBucketName(String),
}

const ARN_PREFIX: &str = "arn";
const S3_SERVICE: &str = "s3";
const ACCESSPOINT: &str = "accesspoint";

impl FromStr for S3Arn {
    type Err = S3ArnError;

    fn from_str(arn: &str) -> Result<Self, Self::Err> {
        let mut parts = arn.splitn(6, ':');
        let arn_part = parts.next().ok_or(S3ArnError::NoPrefix)?;
        if arn_part != ARN_PREFIX {
            return Err(S3ArnError::BadPrefix(arn_part.to_owned()));
        }
        let partition = parts.next().ok_or(S3ArnError::NoPartition)?.to_owned();
        let service = parts.next().ok_or(S3ArnError::NoService)?.to_owned();
        if service != S3_SERVICE {
            return Err(S3ArnError::BadService(service));
        }
        let region = parts.next().ok_or(S3ArnError::NoRegion)?.to_owned();
        let account_id = parts.next().ok_or(S3ArnError::NoAccountId)?.to_owned();
        let resource = parts.next().ok_or(S3ArnError::NoResource)?.to_owned();

        let remainder = parts.map(ToOwned::to_owned).collect::<Vec<_>>();
        if !remainder.is_empty() {
            return Err(S3ArnError::Remainder(remainder));
        }

        let (accesspoint, resource_path) =
            resource.split_once('/').ok_or(S3ArnError::NoAccessPoint)?;
        if accesspoint != ACCESSPOINT {
            return Err(S3ArnError::BadAccessPoint(accesspoint.to_owned()));
        }

        let (bucket_name, bucket_path) =
            if let Some((bucket_name, bucket_path)) = resource_path.split_once('/') {
                (bucket_name.to_owned(), Some(bucket_path.to_owned()))
            } else {
                (resource_path.to_owned(), None)
            };

        if bucket_name.is_empty() {
            return Err(S3ArnError::BadBucketName(bucket_name));
        }

        Ok(Self {
            partition,
            region,
            account_id,
            bucket_name,
            bucket_path,
        })
    }
}

impl S3Arn {
    fn bucket_arn(&self) -> String {
        format!(
            "{ARN_PREFIX}:{partition}:{S3_SERVICE}:{region}:{account_id}:{ACCESSPOINT}/{bucket_name}",
            partition = self.partition,
            region = self.region,
            account_id = self.account_id,
            bucket_name = self.bucket_name
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_BUCKET: &str = "arn:aws:s3:some-region-1:123456789:accesspoint/my-bucket";

    #[test]
    fn s3_arn_from_str_no_path() {
        let arn = S3Arn::from_str(TEST_BUCKET).unwrap();
        assert_eq!(arn.partition, "aws");
        assert_eq!(arn.region, "some-region-1");
        assert_eq!(arn.account_id, "123456789");
        assert_eq!(arn.bucket_name, "my-bucket");
        assert!(arn.bucket_path.is_none());
        assert_eq!(arn.bucket_arn(), TEST_BUCKET);
    }

    #[test]
    fn s3_arn_from_str_with_path() {
        let arn_str = format!("{TEST_BUCKET}/path/to/backup/dir");
        let arn = S3Arn::from_str(&arn_str).unwrap();
        assert_eq!(arn.partition, "aws");
        assert_eq!(arn.region, "some-region-1");
        assert_eq!(arn.account_id, "123456789");
        assert_eq!(arn.bucket_name, "my-bucket");
        assert_eq!(arn.bucket_path, Some("path/to/backup/dir".into()));
        assert_eq!(arn.bucket_arn(), TEST_BUCKET);
    }
}
