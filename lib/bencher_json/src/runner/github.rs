use bencher_valid::{CiId, GitHash, GitHubIssueNumber, GitHubRepositoryId, Url};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const PENDING_INT: i32 = 0;
const POSTED_INT: i32 = 1;
const FAILED_INT: i32 = 2;

const APPROVAL_INT: i32 = 0;
const UNLINKED_INT: i32 = 1;
const REFUSED_INT: i32 = 2;
const EXHAUSTED_INT: i32 = 3;

/// The GitHub Actions context of a detached run on the project's linked repository.
///
/// Bencher creates the run's GitHub check, completes it once the job finishes, and writes the
/// pull request comment, with a token from the Bencher Cloud GitHub App.
#[typeshare::typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewJobGitHub {
    /// The ID of the repository the workflow runs in, which must be the project's linked repository
    pub repository_id: GitHubRepositoryId,
    /// The commit the check is created on: the head of the pull request, or the commit pushed
    pub head_sha: GitHash,
    /// The pull request to comment on. Without one, the run gets a check and no comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<GitHubIssueNumber>,
    /// Names the check and the comment section in place of the project's name. Only the bytes the
    /// check's name keeps are kept.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ci_id: Option<CiId>,
    /// Only comment if a threshold exists for the branch, testbed, and measure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ci_only_thresholds: Option<bool>,
    /// Only start a new comment if an alert is generated. An existing comment is always updated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ci_only_on_alert: Option<bool>,
}

/// Bencher's post of a job's GitHub check and pull request comment
#[typeshare::typeshare]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonJobGitHub {
    pub state: JobGitHubState,
    /// The HTTP status of GitHub's last refusal, if GitHub refused a request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    /// Why the post failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure: Option<JobGitHubFailure>,
    /// Where the owner of the GitHub App's installation approves its permissions, when the post
    /// failed for want of that approval
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_url: Option<Url>,
}

/// The state of a job's GitHub post
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum JobGitHubState {
    /// The check, and the comment if any, are still to be posted.
    Pending = PENDING_INT,
    Posted = POSTED_INT,
    Failed = FAILED_INT,
}

/// Why a job's GitHub post failed
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum JobGitHubFailure {
    /// The installation of the GitHub App has not approved its checks and pull requests write.
    Approval = APPROVAL_INT,
    /// The project is no longer linked to the job's repository.
    Unlinked = UNLINKED_INT,
    /// GitHub refused a request.
    Refused = REFUSED_INT,
    /// Every attempt failed.
    Exhausted = EXHAUSTED_INT,
}

#[cfg(feature = "db")]
mod job_github_db {
    use super::{
        APPROVAL_INT, EXHAUSTED_INT, FAILED_INT, JobGitHubFailure, JobGitHubState, PENDING_INT,
        POSTED_INT, REFUSED_INT, UNLINKED_INT,
    };

    #[derive(Debug, thiserror::Error)]
    pub enum JobGitHubError {
        #[error("Invalid job GitHub state value: {0}")]
        State(i32),
        #[error("Invalid job GitHub failure value: {0}")]
        Failure(i32),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for JobGitHubState
    where
        DB: diesel::backend::Backend,
        i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Pending => PENDING_INT.to_sql(out),
                Self::Posted => POSTED_INT.to_sql(out),
                Self::Failed => FAILED_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for JobGitHubState
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                PENDING_INT => Ok(Self::Pending),
                POSTED_INT => Ok(Self::Posted),
                FAILED_INT => Ok(Self::Failed),
                value => Err(Box::new(JobGitHubError::State(value))),
            }
        }
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for JobGitHubFailure
    where
        DB: diesel::backend::Backend,
        i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Approval => APPROVAL_INT.to_sql(out),
                Self::Unlinked => UNLINKED_INT.to_sql(out),
                Self::Refused => REFUSED_INT.to_sql(out),
                Self::Exhausted => EXHAUSTED_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for JobGitHubFailure
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                APPROVAL_INT => Ok(Self::Approval),
                UNLINKED_INT => Ok(Self::Unlinked),
                REFUSED_INT => Ok(Self::Refused),
                EXHAUSTED_INT => Ok(Self::Exhausted),
                value => Err(Box::new(JobGitHubError::Failure(value))),
            }
        }
    }
}
