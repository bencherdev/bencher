use std::path::{Path, PathBuf};

use bencher_json::{system::config::DataStore as DataStoreConfig, Secret};

use crate::ApiError;

pub type DbConnection = diesel::SqliteConnection;

pub struct Database {
    pub path: PathBuf,
    pub connection: DbConnection,
    pub data_store: Option<DataStore>,
}

pub enum DataStore {
    AwsS3(AwsS3),
}

pub struct AwsS3 {
    client: aws_sdk_s3::Client,
    arn: String,
    path: Option<PathBuf>,
}

impl TryFrom<DataStoreConfig> for DataStore {
    type Error = ApiError;

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
    pub async fn backup(&self, source_path: &Path, file_name: &str) -> Result<(), ApiError> {
        match self {
            Self::AwsS3(aws_s3) => aws_s3.backup(source_path, file_name).await,
        }
    }
}

const ARN_AWS_S3: &str = "arn:aws:s3:";
const COLON: char = ':';
const ACCESSPOINT: &str = ":accesspoint/";

impl AwsS3 {
    fn new(
        access_key_id: String,
        secret_access_key: Secret,
        access_point: &str,
    ) -> Result<Self, ApiError> {
        let credentials =
            aws_sdk_s3::Credentials::new(access_key_id, secret_access_key, None, None, "bencher");
        let credentials_provider =
            aws_credential_types::provider::SharedCredentialsProvider::new(credentials);

        let (region, accesspoint_arn) = access_point
            .trim_start_matches(ARN_AWS_S3)
            .split_once(COLON)
            .ok_or_else(|| ApiError::DataStore(access_point.to_string()))?;

        let config = aws_sdk_s3::Config::builder()
            .credentials_provider(credentials_provider)
            .region(aws_sdk_s3::Region::new(region.to_string()))
            .build();
        let client = aws_sdk_s3::Client::from_conf(config);

        let (account_id, resource) = accesspoint_arn
            .split_once(ACCESSPOINT)
            .ok_or_else(|| ApiError::DataStore(access_point.to_string()))?;

        let (bucket_name, bucket_path) =
            if let Some((bucket_name, bucket_path)) = resource.split_once('/') {
                (bucket_name.to_string(), Some(PathBuf::from(bucket_path)))
            } else {
                (resource.to_string(), None)
            };
        let bucket_arn =
            format!("{ARN_AWS_S3}{region}{COLON}{account_id}{ACCESSPOINT}{bucket_name}");

        Ok(Self {
            client,
            arn: bucket_arn,
            path: bucket_path,
        })
    }

    async fn backup(&self, source_path: &Path, file_name: &str) -> Result<(), ApiError> {
        let key = if let Some(bucket_path) = &self.path {
            bucket_path.join(file_name).to_string_lossy().to_string()
        } else {
            file_name.to_string()
        };

        let body = aws_sdk_s3::types::ByteStream::from_path(source_path)
            .await
            .map_err(|e| ApiError::AwsS3(e.to_string()))?;

        self.client
            .put_object()
            .bucket(self.arn.clone())
            .key(key)
            .body(body)
            .send()
            .await
            .map_err(|e| ApiError::AwsS3(e.to_string()))?;

        Ok(())
    }
}
