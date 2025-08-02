use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use bencher_json::{Secret, system::config::DataStore as DataStoreConfig};

pub type DbConnection = diesel::SqliteConnection;

pub struct Database {
    pub path: PathBuf,
    pub connection: Arc<tokio::sync::Mutex<DbConnection>>,
    pub data_store: Option<DataStore>,
}

#[macro_export]
/// Warning: Do not call `connection_lock!` multiple times in the same line, as it will deadlock.
/// Use the `|conn|` syntax to reuse the same connection multiple times in the same line.
macro_rules! yield_connection_lock {
    ($connection:ident, |$conn:ident| $multi:expr) => {{
        tokio::task::yield_now().await;
        let $conn = &mut *$connection.lock().await;
        $multi
    }};
}

pub enum DataStore {
    AwsS3(AwsS3),
}

pub struct AwsS3 {
    client: aws_sdk_s3::Client,
    arn: String,
    path: Option<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum DataStoreError {
    #[error("Failed to configure data store: {0}")]
    DataStore(String),
    #[error("Failed to use AWS S3: {0}")]
    AwsS3(String),
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

const COLON: char = ':';
const ACCESSPOINT: &str = ":accesspoint/";

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

        let (partition, region, accesspoint_arn) = {
            let mut parts = access_point.splitn(4, COLON);
            let _arn = parts.next();
            let partition = parts
                .next()
                .ok_or_else(|| DataStoreError::DataStore(access_point.to_owned()))?;
            let _s3 = parts.next();
            let remaining = parts
                .next()
                .ok_or_else(|| DataStoreError::DataStore(access_point.to_owned()))?;
            let (region, accesspoint_arn) = remaining
                .split_once(COLON)
                .ok_or_else(|| DataStoreError::DataStore(access_point.to_owned()))?;
            (partition, region, accesspoint_arn)
        };

        let config = aws_sdk_s3::Config::builder()
            .credentials_provider(credentials_provider)
            .region(aws_sdk_s3::config::Region::new(region.to_owned()))
            .build();
        let client = aws_sdk_s3::Client::from_conf(config);

        let (account_id, resource) = accesspoint_arn
            .split_once(ACCESSPOINT)
            .ok_or_else(|| DataStoreError::DataStore(access_point.to_owned()))?;

        let (bucket_name, bucket_path) =
            if let Some((bucket_name, bucket_path)) = resource.split_once('/') {
                (bucket_name.to_owned(), Some(PathBuf::from(bucket_path)))
            } else {
                (resource.to_owned(), None)
            };
        let bucket_arn =
            format!("arn:{partition}:s3:{region}{COLON}{account_id}{ACCESSPOINT}{bucket_name}");

        Ok(Self {
            client,
            arn: bucket_arn,
            path: bucket_path,
        })
    }

    async fn backup(&self, source_path: &Path, file_name: &str) -> Result<(), DataStoreError> {
        let key = if let Some(bucket_path) = &self.path {
            bucket_path.join(file_name).to_string_lossy().to_string()
        } else {
            file_name.to_owned()
        };

        let body = aws_sdk_s3::primitives::ByteStream::from_path(source_path)
            .await
            .map_err(|e| DataStoreError::AwsS3(e.to_string()))?;

        self.client
            .put_object()
            .bucket(self.arn.clone())
            .key(key)
            .body(body)
            .send()
            .await
            .map_err(|e| DataStoreError::AwsS3(e.to_string()))?;

        Ok(())
    }
}
