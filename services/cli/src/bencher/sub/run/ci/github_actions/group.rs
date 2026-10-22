use std::{
    collections::{BTreeMap, HashMap, HashSet, hash_map::Entry},
    time::Duration,
};

use bencher_comment::{CommentTag, GroupComment, ReportComment, Section, SectionCheck};
use bencher_json::{GitHash, ProjectSlug, ProjectUuid, ReportUuid};
use octocrab::{Octocrab, models::CommentId};
use url::Url;

use crate::{cli_eprintln_quietable, cli_println_quietable};

use super::{
    super::SectionReports,
    GitHubError, MAX_LENGTH, comment_error,
    external_id::{ExternalId, Kind},
    section::{SectionKey, by_check_run, by_key},
};

const MAX_ROUNDS: usize = 8;
// Long enough for the checks that other jobs completed meanwhile to show in the list.
const SETTLE: Duration = Duration::from_secs(2);
const PER_PAGE: u8 = 100;
const FORBIDDEN: u16 = 403;
const NOT_FOUND: u16 = 404;
const BOT: &str = "Bot";

/// One job's write of its Project's pull request comment, a section per job.
pub struct GroupWrite<'a, R> {
    pub client: Octocrab,
    pub owner: &'a str,
    pub repo: &'a str,
    pub issue_number: u64,
    pub head_sha: GitHash,
    pub project: ProjectUuid,
    pub project_slug: &'a ProjectSlug,
    pub own: OwnSection<'a>,
    pub author: Author,
    pub reports: &'a R,
    /// The console URL of the job's own report, which the other reports share.
    pub console_url: &'a Url,
    pub require_threshold: bool,
    pub only_on_alert: bool,
    pub log: bool,
}

/// The writer's own section, from its own check and its own report, never from the list.
pub struct OwnSection<'a> {
    pub key: SectionKey,
    pub check: SectionCheck,
    pub report: &'a ReportComment,
}

/// The bot user of the GitHub App that owns the job's own check:
/// the only author whose comments the job reads, takes over, or deletes.
pub struct Author {
    app: u64,
    login: String,
}

/// Whether the group write took the comment, or left it to today's comment for the job alone.
pub enum Outcome {
    Done,
    Fallback,
}

impl<R: SectionReports> GroupWrite<'_, R> {
    pub async fn write(self) -> Result<Outcome, GitHubError> {
        if !self.is_head().await? {
            cli_println_quietable!(
                self.log,
                "The pull request head is no longer {}. Skipping PR comment.",
                self.head_sha
            );
            return Ok(Outcome::Done);
        }
        let listed = match self.list_checks().await {
            Ok(listed) => listed,
            Err(err) => {
                cli_eprintln_quietable!(
                    self.log,
                    "Failed to list the GitHub Checks, so the pull request comment holds this job alone.\n{err}"
                );
                return Ok(Outcome::Fallback);
            },
        };
        let mut state = State {
            listed,
            comments: self.comments().await?,
            deleted: HashSet::new(),
            posted: false,
            rendered_without: HashSet::new(),
        };
        let mut reports = Reports {
            source: self.reports,
            project: self.project_slug,
            console_url: self.console_url,
            fetched: HashMap::new(),
        };
        for _ in 0..MAX_ROUNDS {
            match self.round(&mut reports, &mut state).await {
                Ok(Round::Again) => {},
                Ok(Round::Done) => break,
                Err(err) => {
                    if !state.posted {
                        return Err(err);
                    }
                    cli_eprintln_quietable!(
                        self.log,
                        "Warning: failed to update the pull request comment, so it may miss a section.\n{err}"
                    );
                    break;
                },
            }
        }
        Ok(Outcome::Done)
    }

    /// Write the sections, unless the target already holds them, then read it back.
    async fn round(
        &self,
        reports: &mut Reports<'_, R>,
        state: &mut State,
    ) -> Result<Round, GitHubError> {
        let recorded = state.comments.records(self.project_slug, &self.head_sha);
        let Some(Sections { others, missing }) =
            self.sections(reports, state.listed.clone(), recorded).await
        else {
            cli_println_quietable!(
                self.log,
                "A newer run of the `{}` section is on {}. Skipping PR comment.",
                self.own.key.label(),
                self.head_sha
            );
            return Ok(Round::Done);
        };
        // A report this job could not render before, and can now, makes its last render stale.
        let held = !state.rendered_without.iter().any(|key| reports.has(*key))
            && state
                .comments
                .holds(self.project_slug, &self.head_sha, &self.records(&others));
        if held {
            state.posted = true;
        } else {
            match self.put(reports, &others, state).await? {
                Put::Wrote => {},
                Put::Again => return Ok(Round::Again),
                Put::Stop => return Ok(Round::Done),
            }
            state.rendered_without.clone_from(&missing);
        }
        for old in state.comments.old() {
            if state.deleted.insert(old) {
                self.delete(old).await?;
            }
        }
        // A report that could not be fetched may change the render once it can be.
        if held && missing.is_empty() {
            return Ok(Round::Done);
        }

        tokio::time::sleep(SETTLE).await;
        if !self.read_back(&mut state.comments).await? {
            cli_println_quietable!(
                self.log,
                "The pull request head is no longer {}. Skipping PR comment.",
                self.head_sha
            );
            return Ok(Round::Done);
        }
        state.listed = self.list_checks().await.map_err(GitHubError::CheckRuns)?;
        Ok(Round::Again)
    }

    /// Render the sections and write them to the target, else to a new comment.
    async fn put(
        &self,
        reports: &Reports<'_, R>,
        sections: &BTreeMap<SectionKey, SectionCheck>,
        state: &mut State,
    ) -> Result<Put, GitHubError> {
        let comments = &mut state.comments;
        let group = self.group(reports, sections);
        if self.require_threshold && !group.has_threshold() {
            cli_println_quietable!(
                self.log,
                "No thresholds set. Skipping pull request comment."
            );
            return Ok(Put::Stop);
        }
        let body = group.html(self.require_threshold, MAX_LENGTH);
        if let Some(id) = comments.target_id() {
            if !self.update(id, &body).await? {
                // Another job deleted it since it was read, so the target is found again.
                *comments = self.comments().await?;
                return Ok(Put::Again);
            }
            state.posted = true;
            comments.wrote(id, body);
            return Ok(Put::Wrote);
        }
        if self.only_on_alert && !group.has_alert() {
            cli_println_quietable!(self.log, "No alerts found. Skipping CI integration.");
            return Ok(Put::Stop);
        }
        let created = self.create(&body).await?;
        state.posted = true;
        *comments = self.comments().await?;
        if comments
            .group
            .as_ref()
            .is_some_and(|(lowest, _)| *lowest < created)
        {
            // Another job created one first: write there, with the sections it holds.
            self.delete(created).await?;
            return Ok(Put::Again);
        }
        comments.wrote(created, body);
        Ok(Put::Wrote)
    }

    /// The SHA guard. A number that names an issue has no head,
    /// and a token that cannot read the pull request cannot check it, so both pass.
    async fn is_head(&self) -> Result<bool, GitHubError> {
        let route = format!(
            "/repos/{owner}/{repo}/pulls/{number}",
            owner = self.owner,
            repo = self.repo,
            number = self.issue_number
        );
        match self.client.get::<PullRequest, _, ()>(route, None).await {
            Ok(PullRequest {
                head: PullRequestHead { sha },
            }) => Ok(sha.eq_ignore_ascii_case(self.head_sha.as_ref())),
            Err(err) => {
                if has_status(&err, NOT_FOUND) {
                    Ok(true)
                } else if has_status(&err, FORBIDDEN) {
                    cli_eprintln_quietable!(
                        self.log,
                        "Warning: failed to read the pull request, so the comment is written without checking its head SHA.\n{err}"
                    );
                    Ok(true)
                } else {
                    Err(GitHubError::PullRequest(err))
                }
            },
        }
    }

    /// Every check run on the head SHA, not only the latest of each name,
    /// since two jobs without `--ci-id` share a check name.
    async fn list_checks(&self) -> Result<Vec<SectionCheck>, octocrab::Error> {
        let route = format!(
            "/repos/{owner}/{repo}/commits/{head_sha}/check-runs",
            owner = self.owner,
            repo = self.repo,
            head_sha = self.head_sha
        );
        let mut checks = Vec::new();
        let mut page: u32 = 1;
        loop {
            let CheckRuns { check_runs } = self
                .client
                .get(
                    &route,
                    Some(&CheckRunsQuery {
                        filter: "all",
                        per_page: PER_PAGE,
                        page,
                    }),
                )
                .await?;
            let len = check_runs.len();
            checks.extend(
                check_runs
                    .into_iter()
                    .filter_map(ListedCheckRun::into_check),
            );
            if len < usize::from(PER_PAGE) {
                return Ok(checks);
            }
            page += 1;
        }
    }

    async fn comments(&self) -> Result<Comments, GitHubError> {
        let route = format!(
            "/repos/{owner}/{repo}/issues/{number}/comments",
            owner = self.owner,
            repo = self.repo,
            number = self.issue_number
        );
        let mut comments = Vec::new();
        let mut page: u32 = 1;
        loop {
            let listed: Vec<IssueComment> = self
                .client
                .get(
                    &route,
                    Some(&CommentsQuery {
                        per_page: PER_PAGE,
                        page,
                    }),
                )
                .await
                .map_err(GitHubError::Comments)?;
            let len = listed.len();
            comments.extend(listed);
            if len < usize::from(PER_PAGE) {
                return Ok(Comments::new(comments, self.project_slug, &self.author));
            }
            page += 1;
        }
    }

    /// The other sections, or `None` when the writer is stale.
    /// A `t` section needs its report for a key, so its report is fetched first;
    /// a `c` section's only when it is the newest of its key.
    async fn sections(
        &self,
        reports: &mut Reports<'_, R>,
        listed: Vec<SectionCheck>,
        recorded: Vec<SectionCheck>,
    ) -> Option<Sections> {
        let mut missing = HashSet::new();
        let mut keyed = Vec::new();
        for candidate in by_check_run(listed, recorded, self.project, self.own.check.id) {
            let ExternalId { report, kind, .. } = candidate.external_id;
            let report = if let (Kind::Tag { .. }, Some(uuid)) = (kind, report) {
                let pending = candidate.check.conclusion.is_none();
                let fetched = reports.fetch(uuid, kind, pending).await;
                if fetched.is_none() {
                    missing.insert((uuid, pending));
                }
                fetched
            } else {
                None
            };
            if let Some(key) = SectionKey::new(kind, &candidate.check, report) {
                keyed.push((key, candidate.check));
            }
        }
        let others = by_key(keyed, &self.own.key, self.own.check.id)?;
        for check in others.values() {
            if let Some(ExternalId {
                report: Some(uuid),
                kind: kind @ Kind::CiId,
                ..
            }) = ExternalId::parse(&check.external_id)
                && reports
                    .fetch(uuid, kind, check.conclusion.is_none())
                    .await
                    .is_none()
            {
                missing.insert((uuid, check.conclusion.is_none()));
            }
        }
        Some(Sections { others, missing })
    }

    /// Every section's check run, the writer's own included, in ID order as the target holds them.
    fn records(&self, others: &BTreeMap<SectionKey, SectionCheck>) -> Vec<SectionCheck> {
        let mut records = others
            .values()
            .chain([&self.own.check])
            .cloned()
            .collect::<Vec<_>>();
        records.sort_by_key(|record| record.id);
        records
    }

    fn group<'s>(
        &'s self,
        reports: &'s Reports<'_, R>,
        sections: &BTreeMap<SectionKey, SectionCheck>,
    ) -> GroupComment<'s> {
        let others = sections
            .iter()
            .map(|(key, check)| {
                let report = ExternalId::parse(&check.external_id)
                    .and_then(|external_id| external_id.report)
                    .and_then(|uuid| reports.fetched(uuid, check.conclusion.is_none()));
                Section::new(key.label().to_owned(), check.clone(), report)
            })
            .collect();
        GroupComment::new(
            self.own.key.label().to_owned(),
            self.own.check.clone(),
            self.own.report,
            others,
            self.head_sha.clone(),
        )
    }

    /// `false` when the comment is gone.
    async fn update(&self, id: CommentId, body: &str) -> Result<bool, GitHubError> {
        match self
            .client
            .issues(self.owner, self.repo)
            .update_comment(id, body)
            .await
        {
            Ok(_) => Ok(true),
            Err(err) => {
                if has_status(&err, NOT_FOUND) {
                    Ok(false)
                } else {
                    Err(comment_error(err, GitHubError::UpdateComment))
                }
            },
        }
    }

    async fn create(&self, body: &str) -> Result<CommentId, GitHubError> {
        self.client
            .issues(self.owner, self.repo)
            .create_comment(self.issue_number, body)
            .await
            .map(|comment| comment.id)
            .map_err(|err| comment_error(err, GitHubError::CreateComment))
    }

    /// A comment the token may not delete, or that is gone, is passed over.
    async fn delete(&self, id: CommentId) -> Result<(), GitHubError> {
        match self
            .client
            .issues(self.owner, self.repo)
            .delete_comment(id)
            .await
        {
            Ok(()) => Ok(()),
            Err(err) => {
                if has_status(&err, FORBIDDEN) || has_status(&err, NOT_FOUND) {
                    Ok(())
                } else {
                    Err(GitHubError::DeleteComment(err))
                }
            },
        }
    }

    /// `false` when a job of the pull request's new head SHA wrote the target meanwhile.
    async fn read_back(&self, comments: &mut Comments) -> Result<bool, GitHubError> {
        if let Some(id) = comments.target_id() {
            let route = format!(
                "/repos/{owner}/{repo}/issues/comments/{id}",
                owner = self.owner,
                repo = self.repo,
            );
            match self.client.get::<IssueComment, _, ()>(route, None).await {
                Ok(comment) => comments.wrote(id, comment.body.unwrap_or_default()),
                Err(err) => {
                    if has_status(&err, NOT_FOUND) {
                        *comments = self.comments().await?;
                    } else {
                        return Err(GitHubError::Comment(err));
                    }
                },
            }
        }
        let other_head = comments
            .group
            .as_ref()
            .and_then(|(_, body)| CommentTag::parse(body, self.project_slug))
            .is_some_and(|tag| tag != CommentTag::Group(self.head_sha.clone()));
        Ok(!other_head || self.is_head().await?)
    }
}

impl Author {
    /// `None` without the app's slug, which names its bot user.
    pub fn new(app: CheckApp) -> Option<Self> {
        let CheckApp { id, slug } = app;
        Some(Self {
            app: id,
            login: format!("{}[bot]", slug?),
        })
    }

    /// A person cannot take a login that ends in `[bot]`, and GitHub names the app,
    /// where it names one, by ID.
    fn wrote(&self, comment: &IssueComment) -> bool {
        let by_bot = comment
            .user
            .as_ref()
            .is_some_and(|user| user.kind == BOT && user.login == self.login);
        let via_app = comment
            .performed_via_github_app
            .as_ref()
            .is_none_or(|app| app.id == self.app);
        by_bot && via_app
    }
}

/// The state of a write across its rounds.
struct State {
    listed: Vec<SectionCheck>,
    comments: Comments,
    deleted: HashSet<CommentId>,
    /// Whether a write has landed, or the target held the sections, after which a failure is a warning.
    posted: bool,
    /// The reports, in the state of their checks, that could not be fetched when this job last
    /// wrote the target.
    rendered_without: HashSet<(ReportUuid, bool)>,
}

enum Round {
    Again,
    Done,
}

/// The other sections, and the reports of theirs, in the state of their checks, that could not
/// be fetched.
struct Sections {
    others: BTreeMap<SectionKey, SectionCheck>,
    missing: HashSet<(ReportUuid, bool)>,
}

/// What one write did: wrote the target, found it gone or lost a create race, or stopped at a gate.
enum Put {
    Wrote,
    Again,
    Stop,
}

/// The comments of the Project: the group comment with the lowest ID, and today's comments.
struct Comments {
    group: Option<(CommentId, String)>,
    /// Lowest ID first.
    job: Vec<(CommentId, String)>,
}

impl Comments {
    fn new(comments: Vec<IssueComment>, project: &ProjectSlug, author: &Author) -> Self {
        let mut group: Option<(CommentId, String)> = None;
        let mut job = Vec::new();
        for comment in comments {
            if !author.wrote(&comment) {
                continue;
            }
            let IssueComment { id, body, .. } = comment;
            let Some(body) = body else {
                continue;
            };
            match CommentTag::parse(&body, project) {
                Some(CommentTag::Group(_)) => {
                    if group.as_ref().is_none_or(|(lowest, _)| id < *lowest) {
                        group = Some((id, body));
                    }
                },
                Some(CommentTag::Job) => job.push((id, body)),
                None => {},
            }
        }
        job.sort_by_key(|(id, _)| *id);
        Self { group, job }
    }

    /// The group comment, else today's comment with the lowest ID, which the job takes over.
    fn target_id(&self) -> Option<CommentId> {
        self.group.as_ref().or(self.job.first()).map(|(id, _)| *id)
    }

    /// Today's comments that the group comment replaces.
    fn old(&self) -> Vec<CommentId> {
        self.job.iter().map(|(id, _)| *id).collect()
    }

    fn wrote(&mut self, id: CommentId, body: String) {
        self.job.retain(|(job, _)| *job != id);
        self.group = Some((id, body));
    }

    /// Whether the group comment's tag is for the head SHA and it holds exactly these records.
    fn holds(&self, project: &ProjectSlug, head_sha: &GitHash, records: &[SectionCheck]) -> bool {
        let mut held = self.records(project, head_sha);
        held.sort_by_key(|record| record.id);
        held == records
    }

    /// The check runs that the group comment holds, when its tag is for the head SHA.
    fn records(&self, project: &ProjectSlug, head_sha: &GitHash) -> Vec<SectionCheck> {
        self.group
            .as_ref()
            .filter(|(_, body)| {
                CommentTag::parse(body, project) == Some(CommentTag::Group(head_sha.clone()))
            })
            .map(|(_, body)| SectionCheck::parse_all(body))
            .unwrap_or_default()
    }
}

/// The reports of the other sections, fetched once each per pending state for the whole write.
/// A fetch that fails is tried again in the next round.
struct Reports<'a, R> {
    source: &'a R,
    project: &'a ProjectSlug,
    console_url: &'a Url,
    fetched: HashMap<(ReportUuid, bool), ReportComment>,
}

impl<R: SectionReports> Reports<'_, R> {
    async fn fetch(
        &mut self,
        uuid: ReportUuid,
        kind: Kind,
        pending: bool,
    ) -> Option<&ReportComment> {
        let Self {
            source,
            project,
            console_url,
            fetched,
        } = self;
        match fetched.entry((uuid, pending)) {
            Entry::Occupied(entry) => Some(entry.into_mut()),
            Entry::Vacant(entry) => {
                let report = source
                    .section_report(console_url, project, uuid, kind.into())
                    .await?;
                Some(entry.insert(report.with_pending(pending)))
            },
        }
    }

    fn fetched(&self, uuid: ReportUuid, pending: bool) -> Option<&ReportComment> {
        self.fetched.get(&(uuid, pending))
    }

    /// A report in one state does not stand for the same report in the other.
    fn has(&self, key: (ReportUuid, bool)) -> bool {
        self.fetched.contains_key(&key)
    }
}

// https://docs.github.com/en/rest/checks/runs?apiVersion=2022-11-28#list-check-runs-for-a-git-reference
#[derive(serde::Serialize)]
struct CheckRunsQuery {
    filter: &'static str,
    per_page: u8,
    page: u32,
}

#[derive(serde::Deserialize)]
struct CheckRuns {
    check_runs: Vec<ListedCheckRun>,
}

#[derive(serde::Deserialize)]
struct ListedCheckRun {
    id: u64,
    name: String,
    conclusion: Option<String>,
    external_id: Option<String>,
}

impl ListedCheckRun {
    fn into_check(self) -> Option<SectionCheck> {
        let Self {
            id,
            name,
            conclusion,
            external_id,
        } = self;
        Some(SectionCheck {
            id,
            name,
            conclusion,
            external_id: external_id?,
        })
    }
}

/// The app of a check run, as GitHub returns it.
#[derive(serde::Deserialize)]
pub struct CheckApp {
    id: u64,
    slug: Option<String>,
}

#[derive(serde::Serialize)]
struct CommentsQuery {
    per_page: u8,
    page: u32,
}

// https://docs.github.com/en/rest/issues/comments?apiVersion=2022-11-28#get-an-issue-comment
#[derive(serde::Deserialize)]
struct IssueComment {
    id: CommentId,
    body: Option<String>,
    user: Option<User>,
    performed_via_github_app: Option<GitHubApp>,
}

#[derive(serde::Deserialize)]
struct User {
    login: String,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(serde::Deserialize)]
struct GitHubApp {
    id: u64,
}

#[derive(serde::Deserialize)]
struct PullRequest {
    head: PullRequestHead,
}

#[derive(serde::Deserialize)]
struct PullRequestHead {
    sha: String,
}

fn has_status(err: &octocrab::Error, status: u16) -> bool {
    matches!(err, octocrab::Error::GitHub { source, .. } if source.status_code.as_u16() == status)
}

#[cfg(test)]
mod tests {
    use super::{Author, CheckApp, IssueComment};

    const APP: u64 = 15368;

    fn comment(user: &serde_json::Value, via: Option<u64>) -> IssueComment {
        serde_json::from_value(serde_json::json!({
            "id": 1,
            "body": "",
            "user": user,
            "performed_via_github_app": via.map(|id| serde_json::json!({ "id": id })),
        }))
        .unwrap()
    }

    fn user(login: &str, kind: &str) -> serde_json::Value {
        serde_json::json!({ "login": login, "type": kind })
    }

    // Kills an author check that reads only the login or only the type,
    // or that passes over the app GitHub names by ID.
    #[test]
    fn only_the_bot_of_the_check_s_app_wrote_it() {
        let author = Author::new(CheckApp {
            id: APP,
            slug: Some("github-actions".to_owned()),
        })
        .unwrap();
        let bot = || user("github-actions[bot]", "Bot");
        assert!(author.wrote(&comment(&bot(), None)));
        assert!(author.wrote(&comment(&bot(), Some(APP))));
        assert!(!author.wrote(&comment(&bot(), Some(APP + 1))));
        assert!(!author.wrote(&comment(&user("github-actions[bot]", "User"), None)));
        assert!(!author.wrote(&comment(&user("other-app[bot]", "Bot"), None)));
        assert!(!author.wrote(&comment(&serde_json::Value::Null, None)));
    }
}
