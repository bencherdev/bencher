#![cfg(feature = "plus")]
//! The server's GitHub requests for a detached run: its check, its pull request's head, and its
//! pull request comment. Each call is one request under an installation token, so the caller
//! paces every request.

use bencher_comment::CommentTag;
use bencher_github_client::{GitHubApiError, GitHubApp};
use bencher_json::{
    GitHash, GitHubAppSlug, GitHubCheckRunId, GitHubIssueNumber, GitHubRepositoryName, ProjectSlug,
    Secret,
};
use serde::{Deserialize, Serialize, de::IgnoredAny};

/// GitHub's largest page.
const PER_PAGE: usize = 100;
const BOT: &str = "Bot";
const IN_PROGRESS: &str = "in_progress";

/// One repository, through a token minted for it.
pub struct GitHubPost<'a> {
    app: &'a GitHubApp,
    token: &'a Secret,
    full_name: &'a GitHubRepositoryName,
}

/// A check run's name and summary, and its conclusion once the job has one.
pub struct Check<'a> {
    pub name: &'a str,
    pub summary: &'a str,
    pub conclusion: Option<Conclusion>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conclusion {
    Success,
    Failure,
    Cancelled,
}

impl Conclusion {
    /// The conclusion as GitHub names it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Where the pull request's head is, for the job's commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Head {
    Current,
    Moved,
    /// The number names an issue, or nothing.
    NoPullRequest,
}

/// One page of a pull request's comments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommentsPage {
    /// The project's group comment by the app's bot, if the page holds it.
    pub group: Option<CommentId>,
    /// Whether the next page can hold more comments.
    pub more: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct CommentId(u64);

/// What an update found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Updated {
    Updated,
    /// The comment was deleted since it was read.
    Gone,
}

impl<'a> GitHubPost<'a> {
    pub fn new(app: &'a GitHubApp, token: &'a Secret, full_name: &'a GitHubRepositoryName) -> Self {
        Self {
            app,
            token,
            full_name,
        }
    }

    /// Create a check run on `head_sha`, in progress or with its conclusion.
    pub async fn create_check(
        &self,
        head_sha: &GitHash,
        check: &Check<'_>,
    ) -> Result<GitHubCheckRunId, GitHubApiError> {
        let JsonCheckRun { id } = self
            .app
            .api()
            .post(
                &self.repository(&["check-runs"]),
                self.token.as_ref(),
                &JsonNewCheckRun {
                    head_sha,
                    check: JsonCheck::from(check),
                },
            )
            .await?;
        Ok(id)
    }

    pub async fn update_check(
        &self,
        id: GitHubCheckRunId,
        check: &Check<'_>,
    ) -> Result<(), GitHubApiError> {
        let IgnoredAny = self
            .app
            .api()
            .patch(
                &self.repository(&["check-runs", &id.to_string()]),
                self.token.as_ref(),
                &JsonCheck::from(check),
            )
            .await?;
        Ok(())
    }

    /// The head SHA guard: whether the pull request's head is still `head_sha`.
    pub async fn head(
        &self,
        number: GitHubIssueNumber,
        head_sha: &GitHash,
    ) -> Result<Head, GitHubApiError> {
        let pull_request = self
            .app
            .api()
            .get(
                &self.repository(&["pulls", &number.to_string()]),
                &[],
                self.token.as_ref(),
            )
            .await;
        match pull_request {
            Ok(JsonPullRequest {
                head: JsonPullRequestHead { sha },
            }) => Ok(if sha.eq_ignore_ascii_case(head_sha.as_ref()) {
                Head::Current
            } else {
                Head::Moved
            }),
            Err(GitHubApiError::NotFound) => Ok(Head::NoPullRequest),
            Err(e) => Err(e),
        }
    }

    /// One page of the pull request's comments, which GitHub lists oldest first, so the first
    /// group comment found is the one with the lowest ID.
    pub async fn comments(
        &self,
        number: GitHubIssueNumber,
        page: u32,
        project: &ProjectSlug,
    ) -> Result<CommentsPage, GitHubApiError> {
        let comments: Vec<JsonIssueComment> = self
            .app
            .api()
            .get(
                &self.repository(&["issues", &number.to_string(), "comments"]),
                &[
                    ("per_page", &PER_PAGE.to_string()),
                    ("page", &page.to_string()),
                ],
                self.token.as_ref(),
            )
            .await?;
        let more = comments.len() == PER_PAGE;
        let slug = self.app.slug();
        let group = comments
            .into_iter()
            .find(|comment| {
                comment.by_bot(slug)
                    && comment.body.as_deref().is_some_and(|body| {
                        matches!(CommentTag::parse(body, project), Some(CommentTag::Group(_)))
                    })
            })
            .map(|comment| comment.id);
        Ok(CommentsPage { group, more })
    }

    pub async fn create_comment(
        &self,
        number: GitHubIssueNumber,
        body: &str,
    ) -> Result<(), GitHubApiError> {
        let IgnoredAny = self
            .app
            .api()
            .post(
                &self.repository(&["issues", &number.to_string(), "comments"]),
                self.token.as_ref(),
                &JsonCommentBody { body },
            )
            .await?;
        Ok(())
    }

    pub async fn update_comment(
        &self,
        id: CommentId,
        body: &str,
    ) -> Result<Updated, GitHubApiError> {
        let updated = self
            .app
            .api()
            .patch::<_, IgnoredAny>(
                &self.repository(&["issues", "comments", &id.0.to_string()]),
                self.token.as_ref(),
                &JsonCommentBody { body },
            )
            .await;
        match updated {
            Ok(IgnoredAny) => Ok(Updated::Updated),
            Err(GitHubApiError::NotFound) => Ok(Updated::Gone),
            Err(e) => Err(e),
        }
    }

    fn repository<'p>(&'p self, path: &[&'p str]) -> Vec<&'p str> {
        ["repos", self.full_name.owner(), self.full_name.name()]
            .into_iter()
            .chain(path.iter().copied())
            .collect()
    }
}

impl Serialize for Conclusion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[derive(Serialize)]
struct JsonNewCheckRun<'a> {
    head_sha: &'a GitHash,
    #[serde(flatten)]
    check: JsonCheck<'a>,
}

/// A check run with no `external_id`, so a synchronous run's comment never reads it as its own.
#[derive(Serialize)]
struct JsonCheck<'a> {
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    conclusion: Option<Conclusion>,
    output: JsonOutput<'a>,
}

#[derive(Serialize)]
struct JsonOutput<'a> {
    title: &'static str,
    summary: &'a str,
}

impl<'a> From<&Check<'a>> for JsonCheck<'a> {
    fn from(check: &Check<'a>) -> Self {
        let Check {
            name,
            summary,
            conclusion,
        } = *check;
        Self {
            name,
            // A conclusion completes the check.
            status: conclusion.is_none().then_some(IN_PROGRESS),
            conclusion,
            output: JsonOutput { title: "", summary },
        }
    }
}

#[derive(Deserialize)]
struct JsonCheckRun {
    id: GitHubCheckRunId,
}

#[derive(Deserialize)]
struct JsonPullRequest {
    head: JsonPullRequestHead,
}

#[derive(Deserialize)]
struct JsonPullRequestHead {
    sha: String,
}

#[derive(Serialize)]
struct JsonCommentBody<'a> {
    body: &'a str,
}

#[derive(Deserialize)]
struct JsonIssueComment {
    id: CommentId,
    body: Option<String>,
    user: Option<JsonUser>,
    performed_via_github_app: Option<JsonApp>,
}

#[derive(Deserialize)]
struct JsonUser {
    login: String,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct JsonApp {
    slug: Option<String>,
}

impl JsonIssueComment {
    /// A person cannot take a login that ends in `[bot]`, and GitHub names the app, where it names
    /// one, by its slug.
    fn by_bot(&self, slug: &GitHubAppSlug) -> bool {
        let slug: &str = slug.as_ref();
        let by_bot = self.user.as_ref().is_some_and(|user| {
            user.kind == BOT
                && user
                    .login
                    .strip_suffix("[bot]")
                    .is_some_and(|login| login == slug)
        });
        let via_app = self
            .performed_via_github_app
            .as_ref()
            .is_none_or(|app| app.slug.as_deref() == Some(slug));
        by_bot && via_app
    }
}

#[cfg(test)]
mod tests {
    use bencher_comment::group_tag;
    use bencher_github_client::{
        GitHubApiError, GitHubApp,
        test_server::{Received, Server, app_key, install_crypto_provider},
    };
    use bencher_json::{GitHash, GitHubIssueNumber, ProjectSlug, Secret};

    use super::{Check, CommentId, CommentsPage, Conclusion, GitHubPost, Head, Updated};

    const SLUG: &str = "bencher-test";
    const TOKEN: &str = "ghs_installation-token-marker";
    const SHA: &str = "0123456789abcdef0123456789abcdef01234567";
    const OTHER_SHA: &str = "fedcba9876543210fedcba9876543210fedcba98";
    const NAME: &str = "Bencher Report (the-project)";
    const SUMMARY: &str = "<h2>Bencher Report</h2>";

    struct Fixture {
        server: Server,
        app: GitHubApp,
        token: Secret,
        full_name: bencher_json::GitHubRepositoryName,
    }

    impl Fixture {
        async fn new(responses: Vec<(u16, String)>) -> Self {
            install_crypto_provider();
            let server = Server::start(responses).await;
            let (private_key, _public_key) = app_key();
            let app = GitHubApp::new(
                "Iv1.0123456789abcdef".parse().unwrap(),
                SLUG.parse().unwrap(),
                &private_key,
                "webhook-secret-marker".parse().unwrap(),
            )
            .unwrap()
            .with_base_url(server.base())
            .unwrap();
            Self {
                server,
                app,
                token: TOKEN.parse().unwrap(),
                full_name: "octo-cat/hello".parse().unwrap(),
            }
        }

        fn post(&self) -> GitHubPost<'_> {
            GitHubPost::new(&self.app, &self.token, &self.full_name)
        }

        /// Every request, each of which carried the installation token.
        async fn received(self) -> Vec<Received> {
            let received = self.server.received().await;
            for request in &received {
                assert_eq!(
                    request.header("authorization"),
                    Some(format!("Bearer {TOKEN}").as_str()),
                    "{}",
                    request.request_line()
                );
            }
            received
        }
    }

    fn sha() -> GitHash {
        SHA.parse().unwrap()
    }

    fn number() -> GitHubIssueNumber {
        GitHubIssueNumber::try_from(7).unwrap()
    }

    fn project() -> ProjectSlug {
        "the-project".parse().unwrap()
    }

    fn check(conclusion: Option<Conclusion>) -> Check<'static> {
        Check {
            name: NAME,
            summary: SUMMARY,
            conclusion,
        }
    }

    // Kills a check sent to any other path or under any other token, with an `external_id`, a
    // started check without its in-progress status, a completed one that still sends a status,
    // and an ID read from anywhere but the answer.
    #[tokio::test]
    async fn a_check_starts_in_progress_and_completes_with_no_external_id() {
        let fixture = Fixture::new(vec![
            (201, r#"{"id": 11, "external_id": null}"#.to_owned()),
            (200, r#"{"id": 11}"#.to_owned()),
            (201, r#"{"id": 12}"#.to_owned()),
        ])
        .await;
        let post = fixture.post();

        let started = post.create_check(&sha(), &check(None)).await.unwrap();
        post.update_check(started, &check(Some(Conclusion::Failure)))
            .await
            .unwrap();
        let created = post
            .create_check(&sha(), &check(Some(Conclusion::Cancelled)))
            .await
            .unwrap();

        assert_eq!(
            (u64::from(started), u64::from(created)),
            (11, 12),
            "the IDs GitHub gave"
        );
        let output = serde_json::json!({ "title": "", "summary": SUMMARY });
        let received = fixture.received().await;
        let sent: Vec<(&str, serde_json::Value)> = received
            .iter()
            .map(|request| (request.request_line(), request.json()))
            .collect();
        assert_eq!(
            sent,
            [
                (
                    "POST /github/repos/octo-cat/hello/check-runs HTTP/1.1",
                    serde_json::json!({
                        "name": NAME,
                        "head_sha": SHA,
                        "status": "in_progress",
                        "output": output,
                    }),
                ),
                (
                    "PATCH /github/repos/octo-cat/hello/check-runs/11 HTTP/1.1",
                    serde_json::json!({
                        "name": NAME,
                        "conclusion": "failure",
                        "output": output,
                    }),
                ),
                (
                    "POST /github/repos/octo-cat/hello/check-runs HTTP/1.1",
                    serde_json::json!({
                        "name": NAME,
                        "head_sha": SHA,
                        "conclusion": "cancelled",
                        "output": output,
                    }),
                ),
            ]
        );
    }

    // Kills a guard that compares the SHA's case, passes a moved head, reads a 404 as anything
    // but no pull request, or reads any other refusal as an answer.
    #[tokio::test]
    async fn the_head_guard_reads_the_pull_request() {
        let pull_request = |sha: &str| format!(r#"{{"number": 7, "head": {{"sha": "{sha}"}}}}"#);
        let fixture = Fixture::new(vec![
            (200, pull_request(&SHA.to_uppercase())),
            (200, pull_request(OTHER_SHA)),
            (404, r#"{"message": "Not Found"}"#.to_owned()),
            (500, r#"{"message": "Server Error"}"#.to_owned()),
        ])
        .await;
        let post = fixture.post();

        let mut heads = Vec::new();
        for _ in 0..3 {
            heads.push(post.head(number(), &sha()).await.unwrap());
        }
        let error = post.head(number(), &sha()).await.unwrap_err();

        assert_eq!(heads, [Head::Current, Head::Moved, Head::NoPullRequest]);
        assert!(
            matches!(error, GitHubApiError::Status(status, _) if status.as_u16() == 500),
            "{error:?}"
        );
        for request in fixture.received().await {
            assert_eq!(
                request.request_line(),
                "GET /github/repos/octo-cat/hello/pulls/7 HTTP/1.1"
            );
        }
    }

    // Kills finding a comment by any author but the app's bot, by its bot through another app, a
    // comment of the job tag or of another project, a comment that only holds the tag before its
    // end, and a later comment over the first; and a page count that reads a short page as full.
    #[tokio::test]
    async fn only_the_app_bot_s_group_comment_of_the_project_is_found() {
        let tag = group_tag(&project(), &sha());
        let other_project = group_tag(&"the-project-2".parse().unwrap(), &sha());
        let comment = |id: u64, login: &str, kind: &str, via: Option<&str>, body: &str| {
            serde_json::json!({
                "id": id,
                "body": body,
                "user": { "login": login, "type": kind },
                "performed_via_github_app": via.map(|slug| serde_json::json!({ "id": 1, "slug": slug })),
            })
        };
        let bot = "bencher-test[bot]";
        let page = serde_json::json!([
            comment(1, "octo-cat", "User", None, &tag),
            comment(
                2,
                "github-actions[bot]",
                "Bot",
                Some("github-actions"),
                &tag
            ),
            comment(3, bot, "User", None, &tag),
            comment(4, bot, "Bot", Some("other-app"), &tag),
            comment(5, "other-app[bot]", "Bot", Some(SLUG), &tag),
            comment(6, bot, "Bot", Some(SLUG), &other_project),
            comment(7, bot, "Bot", Some(SLUG), &format!("{tag}<p>edited</p>")),
            comment(
                8,
                bot,
                "Bot",
                Some(SLUG),
                "<div id=\"bencher.dev/projects/the-project/id/main\"></div>"
            ),
            comment(9, bot, "Bot", Some(SLUG), &format!("<h2>Report</h2>{tag}")),
            comment(10, bot, "Bot", None, &tag),
        ]);
        let full = serde_json::Value::Array(
            (100..200)
                .map(|id| comment(id, "octo-cat", "User", None, ""))
                .collect(),
        );
        let fixture = Fixture::new(vec![
            (200, page.to_string()),
            (200, full.to_string()),
            (200, "[]".to_owned()),
        ])
        .await;
        let post = fixture.post();

        let found = post.comments(number(), 1, &project()).await.unwrap();
        let next = post.comments(number(), 2, &project()).await.unwrap();
        let last = post.comments(number(), 3, &project()).await.unwrap();

        assert_eq!(
            found,
            CommentsPage {
                group: Some(CommentId(9)),
                more: false,
            }
        );
        assert_eq!(
            next,
            CommentsPage {
                group: None,
                more: true,
            }
        );
        assert_eq!(
            last,
            CommentsPage {
                group: None,
                more: false,
            }
        );
        let lines: Vec<String> = fixture
            .received()
            .await
            .iter()
            .map(|request| request.request_line().to_owned())
            .collect();
        assert_eq!(
            lines,
            [1, 2, 3].map(|page| format!(
                "GET /github/repos/octo-cat/hello/issues/7/comments?per_page=100&page={page} HTTP/1.1"
            ))
        );
    }

    // Kills a write to any other path, with any other body, and an update that reads a deleted
    // comment as written or any other refusal as deleted.
    #[tokio::test]
    async fn a_comment_is_created_or_updated_and_a_deleted_one_is_gone() {
        let fixture = Fixture::new(vec![
            (201, r#"{"id": 21}"#.to_owned()),
            (200, r#"{"id": 21}"#.to_owned()),
            (404, r#"{"message": "Not Found"}"#.to_owned()),
            (
                403,
                r#"{"message": "Resource not accessible by integration"}"#.to_owned(),
            ),
        ])
        .await;
        let post = fixture.post();

        post.create_comment(number(), "first").await.unwrap();
        let updated = post.update_comment(CommentId(21), "second").await.unwrap();
        let gone = post.update_comment(CommentId(21), "third").await.unwrap();
        let refused = post
            .update_comment(CommentId(21), "fourth")
            .await
            .unwrap_err();

        assert_eq!((updated, gone), (Updated::Updated, Updated::Gone));
        assert!(
            matches!(refused, GitHubApiError::Status(status, _) if status.as_u16() == 403),
            "{refused:?}"
        );
        let received = fixture.received().await;
        let sent: Vec<(&str, serde_json::Value)> = received
            .iter()
            .map(|request| (request.request_line(), request.json()))
            .collect();
        assert_eq!(
            sent,
            [
                (
                    "POST /github/repos/octo-cat/hello/issues/7/comments HTTP/1.1",
                    serde_json::json!({ "body": "first" }),
                ),
                (
                    "PATCH /github/repos/octo-cat/hello/issues/comments/21 HTTP/1.1",
                    serde_json::json!({ "body": "second" }),
                ),
                (
                    "PATCH /github/repos/octo-cat/hello/issues/comments/21 HTTP/1.1",
                    serde_json::json!({ "body": "third" }),
                ),
                (
                    "PATCH /github/repos/octo-cat/hello/issues/comments/21 HTTP/1.1",
                    serde_json::json!({ "body": "fourth" }),
                ),
            ]
        );
    }
}
