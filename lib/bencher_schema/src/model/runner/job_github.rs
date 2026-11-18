use bencher_comment::truncate_id;
use bencher_json::{
    CiId, DateTime, GitHash, GitHubCheckRunId, GitHubIssueNumber, GitHubRepositoryId, JobStatus,
    Url,
    runner::{
        CALLBACK_JOB_STATUSES, JobGitHubFailure, JobGitHubState, JsonJobGitHub, JsonNewJobGitHub,
    },
};
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _,
    SelectableHelper as _, result::QueryResult,
};
use http::StatusCode;

use super::{HttpStatus, JobId};
use crate::{
    context::DbConnection,
    schema::{self, job_github as job_github_table},
};

/// A detached run's GitHub Actions context, and Bencher's post of its check and comment.
#[derive(Debug, Clone, diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = job_github_table)]
pub struct QueryJobGitHub {
    pub job_id: JobId,
    pub repository_id: GitHubRepositoryId,
    pub head_sha: GitHash,
    pub number: Option<GitHubIssueNumber>,
    pub ci_id: Option<CiId>,
    pub ci_only_thresholds: bool,
    pub ci_only_on_alert: bool,
    pub check_run_id: Option<GitHubCheckRunId>,
    pub state: JobGitHubState,
    pub attempts: i32,
    pub status: Option<HttpStatus>,
    pub failure: Option<JobGitHubFailure>,
    pub approval_url: Option<Url>,
    pub created: DateTime,
    pub modified: DateTime,
}

impl QueryJobGitHub {
    pub fn get(conn: &mut DbConnection, job_id: JobId) -> QueryResult<Self> {
        schema::job_github::table
            .filter(schema::job_github::job_id.eq(job_id))
            .select(Self::as_select())
            .first(conn)
    }

    /// Record the check a post created, unless one is recorded already.
    pub fn record_check(
        conn: &mut DbConnection,
        job_id: JobId,
        check_run_id: GitHubCheckRunId,
        now: DateTime,
    ) -> QueryResult<()> {
        diesel::update(
            schema::job_github::table
                .filter(schema::job_github::job_id.eq(job_id))
                .filter(schema::job_github::state.eq(JobGitHubState::Pending))
                .filter(schema::job_github::check_run_id.is_null()),
        )
        .set((
            schema::job_github::check_run_id.eq(check_run_id),
            schema::job_github::modified.eq(now),
        ))
        .execute(conn)
        .map(drop)
    }

    /// Count an attempt on a pending post, and keep its status if GitHub refused it. With an
    /// outcome, the attempt ends the post, and the same update settles it.
    pub fn record_attempt(
        conn: &mut DbConnection,
        job_id: JobId,
        status: Option<StatusCode>,
        outcome: Option<PostOutcome>,
        now: DateTime,
    ) -> QueryResult<bool> {
        diesel::update(
            schema::job_github::table
                .filter(schema::job_github::job_id.eq(job_id))
                .filter(schema::job_github::state.eq(JobGitHubState::Pending)),
        )
        .set((
            schema::job_github::attempts.eq(schema::job_github::attempts + 1),
            status.map(|status| schema::job_github::status.eq(HttpStatus::from(status))),
            outcome.map(Settle::from),
            schema::job_github::modified.eq(now),
        ))
        .execute(conn)
        .map(|updated| updated > 0)
    }

    /// Settle a pending post without an attempt, and keep the status of GitHub's refusal if one
    /// settled it.
    pub fn finish(
        conn: &mut DbConnection,
        job_id: JobId,
        outcome: PostOutcome,
        status: Option<StatusCode>,
        now: DateTime,
    ) -> QueryResult<bool> {
        diesel::update(
            schema::job_github::table
                .filter(schema::job_github::job_id.eq(job_id))
                .filter(schema::job_github::state.eq(JobGitHubState::Pending)),
        )
        .set((
            status.map(|status| schema::job_github::status.eq(HttpStatus::from(status))),
            Settle::from(outcome),
            schema::job_github::modified.eq(now),
        ))
        .execute(conn)
        .map(|updated| updated > 0)
    }

    /// The pending posts that a start could have left undone, of a terminal job or of a job whose
    /// check was never started, with each job's status.
    pub fn pending(conn: &mut DbConnection) -> QueryResult<Vec<(JobId, JobStatus)>> {
        schema::job_github::table
            .inner_join(schema::job::table)
            .filter(schema::job_github::state.eq(JobGitHubState::Pending))
            .filter(
                schema::job::status
                    .eq_any(CALLBACK_JOB_STATUSES)
                    .or(schema::job_github::check_run_id.is_null()),
            )
            .order(schema::job_github::job_id.asc())
            .select((schema::job_github::job_id, schema::job::status))
            .load(conn)
    }
}

/// The columns that settle a post.
#[derive(diesel::AsChangeset)]
#[diesel(table_name = job_github_table, treat_none_as_null = true)]
struct Settle {
    state: JobGitHubState,
    failure: Option<JobGitHubFailure>,
    approval_url: Option<Url>,
}

impl From<PostOutcome> for Settle {
    fn from(outcome: PostOutcome) -> Self {
        match outcome {
            PostOutcome::Posted => Self {
                state: JobGitHubState::Posted,
                failure: None,
                approval_url: None,
            },
            PostOutcome::Failed {
                failure,
                approval_url,
            } => Self {
                state: JobGitHubState::Failed,
                failure: Some(failure),
                approval_url,
            },
        }
    }
}

/// A post's state, without the job's context, so a job list loads only what it shows.
#[derive(Debug, Clone, diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = job_github_table)]
pub struct QueryJobGitHubView {
    pub state: JobGitHubState,
    pub status: Option<HttpStatus>,
    pub failure: Option<JobGitHubFailure>,
    pub approval_url: Option<Url>,
}

impl From<QueryJobGitHubView> for JsonJobGitHub {
    fn from(view: QueryJobGitHubView) -> Self {
        let QueryJobGitHubView {
            state,
            status,
            failure,
            approval_url,
        } = view;
        Self {
            state,
            status: status.map(u16::from),
            failure,
            approval_url,
        }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = job_github_table)]
pub struct InsertJobGitHub {
    job_id: JobId,
    repository_id: GitHubRepositoryId,
    head_sha: GitHash,
    number: Option<GitHubIssueNumber>,
    ci_id: Option<CiId>,
    ci_only_thresholds: bool,
    ci_only_on_alert: bool,
    state: JobGitHubState,
    created: DateTime,
    modified: DateTime,
}

impl InsertJobGitHub {
    pub fn pending(job_id: JobId, github: JsonNewJobGitHub, now: DateTime) -> Self {
        let JsonNewJobGitHub {
            repository_id,
            head_sha,
            number,
            ci_id,
            ci_only_thresholds,
            ci_only_on_alert,
        } = github;
        Self {
            job_id,
            repository_id,
            head_sha,
            number,
            // The section a `ci_id` keys must be the one its check's name holds.
            ci_id: ci_id.map(|ci_id| CiId::from(truncate_id(ci_id.as_ref()).to_owned())),
            ci_only_thresholds: ci_only_thresholds.unwrap_or_default(),
            ci_only_on_alert: ci_only_on_alert.unwrap_or_default(),
            state: JobGitHubState::Pending,
            created: now,
            modified: now,
        }
    }

    pub fn insert(&self, conn: &mut DbConnection) -> QueryResult<()> {
        diesel::insert_into(schema::job_github::table)
            .values(self)
            .execute(conn)
            .map(|_| ())
    }
}

/// How a pending post ends.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostOutcome {
    Posted,
    Failed {
        failure: JobGitHubFailure,
        approval_url: Option<Url>,
    },
}

impl PostOutcome {
    pub fn failed(failure: JobGitHubFailure) -> Self {
        Self::Failed {
            failure,
            approval_url: None,
        }
    }
}
