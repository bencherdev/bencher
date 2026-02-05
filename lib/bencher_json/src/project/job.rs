use std::collections::HashMap;

use bencher_valid::{ImageDigest, Url};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ProjectUuid;

crate::typed_uuid::typed_uuid!(JobUuid);

/// A job specification for running a benchmark in an OCI container.
///
/// The runner uses this specification to pull the image, configure the VM,
/// and execute the benchmark.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonJobSpec {
    /// The OCI registry URL where the image is hosted.
    pub registry: Url,
    /// The project UUID that owns the image.
    pub project: ProjectUuid,
    /// The image digest (e.g., "sha256:...").
    pub digest: ImageDigest,
    /// Optional entrypoint override for the container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<Vec<String>>,
    /// Optional command override for the container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cmd: Option<Vec<String>>,
    /// Optional environment variables for the container.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    /// Number of vCPUs to allocate.
    pub vcpu: u32,
    /// Memory size in bytes.
    pub memory: u64,
    /// Disk size in bytes.
    pub disk: u64,
    /// Timeout in seconds.
    pub timeout: u32,
    /// Whether to enable network access.
    pub network: bool,
}

/// The status of a job in the queue.
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    /// Job is waiting to be claimed by a runner.
    Pending,
    /// Job has been claimed by a runner.
    Claimed,
    /// Job is currently executing.
    Running,
    /// Job completed successfully.
    Completed,
    /// Job failed during execution.
    Failed,
    /// Job was canceled by the user.
    Canceled,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Claimed => write!(f, "claimed"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Canceled => write!(f, "canceled"),
        }
    }
}

impl std::str::FromStr for JobStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "claimed" => Ok(Self::Claimed),
            "running" => Ok(Self::Running),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "canceled" => Ok(Self::Canceled),
            _ => Err(format!("Invalid job status: {s}")),
        }
    }
}

#[cfg(feature = "db")]
mod job_status_db {
    use super::JobStatus;

    #[derive(Debug, thiserror::Error)]
    pub enum JobStatusError {
        #[error("Invalid job status value: {0}")]
        Invalid(String),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Text, DB> for JobStatus
    where
        DB: diesel::backend::Backend,
        for<'a> String: diesel::serialize::ToSql<diesel::sql_types::Text, DB>
            + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            out.set_value(self.to_string());
            Ok(diesel::serialize::IsNull::No)
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Text, DB> for JobStatus
    where
        DB: diesel::backend::Backend,
        String: diesel::deserialize::FromSql<diesel::sql_types::Text, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            let status = String::from_sql(bytes)?;
            status.parse().map_err(|e: String| {
                let err: Box<dyn std::error::Error + Send + Sync> =
                    Box::new(JobStatusError::Invalid(e));
                err
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_status_display() {
        assert_eq!(JobStatus::Pending.to_string(), "pending");
        assert_eq!(JobStatus::Claimed.to_string(), "claimed");
        assert_eq!(JobStatus::Running.to_string(), "running");
        assert_eq!(JobStatus::Completed.to_string(), "completed");
        assert_eq!(JobStatus::Failed.to_string(), "failed");
        assert_eq!(JobStatus::Canceled.to_string(), "canceled");
    }

    #[test]
    fn job_status_from_str() {
        assert_eq!("pending".parse::<JobStatus>().unwrap(), JobStatus::Pending);
        assert_eq!("claimed".parse::<JobStatus>().unwrap(), JobStatus::Claimed);
        assert_eq!("running".parse::<JobStatus>().unwrap(), JobStatus::Running);
        assert_eq!(
            "completed".parse::<JobStatus>().unwrap(),
            JobStatus::Completed
        );
        assert_eq!("failed".parse::<JobStatus>().unwrap(), JobStatus::Failed);
        assert_eq!(
            "canceled".parse::<JobStatus>().unwrap(),
            JobStatus::Canceled
        );
        assert!("invalid".parse::<JobStatus>().is_err());
    }

    #[test]
    fn json_job_spec_serde() {
        let spec = JsonJobSpec {
            registry: "https://registry.bencher.dev".parse().unwrap(),
            project: "550e8400-e29b-41d4-a716-446655440000".parse().unwrap(),
            digest: "sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
                .parse()
                .unwrap(),
            entrypoint: Some(vec!["/bin/sh".to_owned()]),
            cmd: Some(vec!["-c".to_owned(), "cargo bench".to_owned()]),
            env: Some([("RUST_LOG".to_owned(), "debug".to_owned())].into()),
            vcpu: 2,
            memory: 0x4000_0000, // 1 GiB (1073741824 bytes)
            disk: 0x2_8000_0000, // 10 GiB (10737418240 bytes)
            timeout: 300,
            network: false,
        };

        let json = serde_json::to_string(&spec).unwrap();
        let parsed: JsonJobSpec = serde_json::from_str(&json).unwrap();

        assert_eq!(spec.registry, parsed.registry);
        assert_eq!(spec.project, parsed.project);
        assert_eq!(spec.digest, parsed.digest);
        assert_eq!(spec.vcpu, parsed.vcpu);
        assert_eq!(spec.memory, parsed.memory);
        assert_eq!(spec.disk, parsed.disk);
        assert_eq!(spec.timeout, parsed.timeout);
        assert_eq!(spec.network, parsed.network);
    }

    #[test]
    fn json_job_spec_minimal() {
        let json = r#"{
            "registry": "https://registry.bencher.dev",
            "project": "550e8400-e29b-41d4-a716-446655440000",
            "digest": "sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3",
            "vcpu": 1,
            "memory": 536870912,
            "disk": 1073741824,
            "timeout": 60,
            "network": false
        }"#;

        let spec: JsonJobSpec = serde_json::from_str(json).unwrap();
        assert!(spec.entrypoint.is_none());
        assert!(spec.cmd.is_none());
        assert!(spec.env.is_none());
    }
}
