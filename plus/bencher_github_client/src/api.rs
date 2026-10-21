use std::{sync::LazyLock, time::Duration};

use bencher_valid::{
    GitHubInstallationId, GitHubInstallationPermission, GitHubRepositoryId, GitHubRepositoryName,
    ValidError,
};
use oauth2::AccessToken;
use reqwest::{
    RequestBuilder, Response, StatusCode,
    header::{ACCEPT, HeaderMap, HeaderName, HeaderValue, LOCATION},
    redirect,
};
use serde::{
    Deserialize, Serialize,
    de::{DeserializeOwned, IgnoredAny},
};
use url::Url;

#[expect(
    clippy::expect_used,
    reason = "constant URL literal, infallible at runtime"
)]
pub(crate) static GITHUB_API_URL: LazyLock<Url> = LazyLock::new(|| {
    "https://api.github.com"
        .parse()
        .expect("Invalid GitHub API URL")
});

pub(crate) const USER_AGENT: &str = concat!("Bencher/", env!("CARGO_PKG_VERSION"));
/// GitHub's largest page.
const PER_PAGE: usize = 100;
pub(crate) const TIMEOUT: Duration = Duration::from_secs(10);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// From GitHub's 422 for a repository outside the installation.
const REPOSITORY_INACCESSIBLE: &str = "not accessible to the parent installation";
/// From GitHub's 422 for a permission the installation has not approved.
const PERMISSIONS_NOT_GRANTED: &str = "permissions requested are not granted";
/// What a 403 or a 429 says about when to try again.
const RATE_LIMIT_HEADERS: [&str; 3] = ["retry-after", "x-ratelimit-remaining", "x-ratelimit-reset"];

/// The GitHub REST API, one request per call. It follows no redirect but one, to the API's own
/// origin, so a credential only ever goes to the API.
#[derive(Debug, Clone)]
pub(crate) struct GitHubApi {
    http: reqwest::Client,
    base: Url,
}

#[derive(Debug, thiserror::Error)]
pub enum GitHubApiError {
    #[error("Failed to send a GitHub API request: {0}")]
    Request(reqwest::Error),
    #[error("The GitHub API found no such resource")]
    NotFound,
    #[error("The GitHub App installation cannot access the repository")]
    RepositoryInaccessible,
    #[error("The GitHub App installation has not granted the permissions requested")]
    PermissionsNotGranted,
    /// Any other answer, and for a 403 or a 429, its rate-limit headers.
    #[error("The GitHub API answered {0}")]
    Status(StatusCode, HeaderMap),
    #[error("Failed to parse a GitHub API response: {0}")]
    Json(reqwest::Error),
    #[error("Failed to parse a GitHub repository name: {0}")]
    RepositoryName(ValidError),
}

impl GitHubApi {
    pub(crate) fn new(base: Url) -> Result<Self, reqwest::Error> {
        let headers = HeaderMap::from_iter([
            (
                ACCEPT,
                HeaderValue::from_static("application/vnd.github+json"),
            ),
            (
                HeaderName::from_static("x-github-api-version"),
                HeaderValue::from_static("2022-11-28"),
            ),
        ]);
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .user_agent(USER_AGENT)
            .redirect(redirect::Policy::none())
            .retry(reqwest::retry::never())
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(TIMEOUT)
            .build()?;
        Ok(Self { http, base })
    }

    /// Every repository the user can push to, in every installation of the app the user can see
    /// that is not suspended.
    pub(crate) async fn user_repositories(
        &self,
        token: &AccessToken,
    ) -> Result<Vec<GitHubRepository>, GitHubApiError> {
        let installations: Vec<JsonInstallation> =
            self.every_page(&["user", "installations"], token).await?;
        let mut pushable = Vec::new();
        // A suspended installation mints no token, so none of its repositories can be linked.
        for JsonInstallation {
            id: installation,
            html_url: installation_url,
            suspended_at: _,
            permissions: JsonInstallationPermissions { contents },
        } in installations
            .into_iter()
            .filter(|installation| installation.suspended_at.is_none())
        {
            let permission = if contents == Some(JsonAccess::Write) {
                GitHubInstallationPermission::Granted
            } else {
                GitHubInstallationPermission::ApprovalRequired
            };
            let repositories: Vec<JsonRepository> = self
                .every_page(
                    &[
                        "user",
                        "installations",
                        &installation.to_string(),
                        "repositories",
                    ],
                    token,
                )
                .await?;
            for JsonRepository {
                id,
                owner: JsonOwner { login },
                name,
                permissions: JsonRepositoryPermissions { push },
            } in repositories
            {
                if push {
                    pushable.push(GitHubRepository {
                        installation,
                        installation_url: installation_url.clone(),
                        permission,
                        id,
                        full_name: GitHubRepositoryName::new(&login, &name)
                            .map_err(GitHubApiError::RepositoryName)?,
                    });
                }
            }
        }
        Ok(pushable)
    }

    /// GET `path`, following one redirect to the API's own origin, which is how GitHub answers for
    /// a repository that was renamed or transferred.
    pub(crate) async fn get_following_redirect<T>(
        &self,
        path: &[&str],
        bearer: &str,
    ) -> Result<T, GitHubApiError>
    where
        T: DeserializeOwned,
    {
        let response = self
            .http
            .get(self.url(path))
            .bearer_auth(bearer)
            .send()
            .await
            .map_err(GitHubApiError::Request)?;
        if response.status().is_redirection()
            && let Some(location) = self.same_origin(&response)
        {
            return send(self.http.get(location).bearer_auth(bearer)).await;
        }
        answer(response).await
    }

    pub(crate) async fn post<B, T>(
        &self,
        path: &[&str],
        bearer: &str,
        body: &B,
    ) -> Result<T, GitHubApiError>
    where
        B: Serialize,
        T: DeserializeOwned,
    {
        send(
            self.http
                .post(self.url(path))
                .bearer_auth(bearer)
                .json(body),
        )
        .await
    }

    async fn every_page<T>(
        &self,
        path: &[&str],
        token: &AccessToken,
    ) -> Result<Vec<T>, GitHubApiError>
    where
        T: DeserializeOwned,
    {
        let mut items = Vec::new();
        for page in 1u32.. {
            let mut url = self.url(path);
            url.query_pairs_mut()
                .append_pair("per_page", &PER_PAGE.to_string())
                .append_pair("page", &page.to_string());
            let JsonPage { items: page_items } =
                send(self.http.get(url).bearer_auth(token.secret())).await?;
            let last = page_items.len() < PER_PAGE;
            items.extend(page_items);
            if last {
                break;
            }
        }
        Ok(items)
    }

    pub(crate) fn url(&self, path: &[&str]) -> Url {
        join_path(&self.base, path)
    }

    /// Where a redirect points, if it points to the API's own origin.
    fn same_origin(&self, response: &Response) -> Option<Url> {
        let location = response.headers().get(LOCATION)?.to_str().ok()?;
        let url = response.url().join(location).ok()?;
        (url.origin() == self.base.origin()).then_some(url)
    }
}

async fn send<T>(request: RequestBuilder) -> Result<T, GitHubApiError>
where
    T: DeserializeOwned,
{
    let response = request.send().await.map_err(GitHubApiError::Request)?;
    answer(response).await
}

async fn answer<T>(response: Response) -> Result<T, GitHubApiError>
where
    T: DeserializeOwned,
{
    let status = response.status();
    let rate_limit = rate_limit(status, response.headers());
    if status.is_success() {
        response.json().await.map_err(GitHubApiError::Json)
    } else if status == StatusCode::NOT_FOUND {
        Err(GitHubApiError::NotFound)
    } else if status == StatusCode::UNPROCESSABLE_ENTITY
        && let Some(refusal) = refusal(response).await
    {
        Err(refusal)
    } else {
        Err(GitHubApiError::Status(status, rate_limit))
    }
}

/// The rate-limit headers of a 403 or a 429, and none of any other answer.
fn rate_limit(status: StatusCode, headers: &HeaderMap) -> HeaderMap {
    if status != StatusCode::FORBIDDEN && status != StatusCode::TOO_MANY_REQUESTS {
        return HeaderMap::new();
    }
    RATE_LIMIT_HEADERS
        .into_iter()
        .filter_map(|name| {
            let value = headers.get(name)?.clone();
            Some((HeaderName::from_static(name), value))
        })
        .collect()
}

/// GitHub tells its 422 refusals apart only in the message, and no refusal holds a token.
async fn refusal(response: Response) -> Option<GitHubApiError> {
    let JsonRefusal { message } = response.json().await.ok()?;
    if message.contains(REPOSITORY_INACCESSIBLE) {
        Some(GitHubApiError::RepositoryInaccessible)
    } else if message.contains(PERMISSIONS_NOT_GRANTED) {
        Some(GitHubApiError::PermissionsNotGranted)
    } else {
        None
    }
}

/// `base` with each of `path` appended as one escaped path segment.
pub(crate) fn join_path(base: &Url, path: &[&str]) -> Url {
    let mut url = base.clone();
    // Only a URL that cannot be a base refuses segments, and reqwest sends nothing to one.
    if let Ok(mut segments) = url.path_segments_mut() {
        segments.pop_if_empty().extend(path);
    }
    url
}

/// A repository the user can push to, in an installation of the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubRepository {
    pub installation: GitHubInstallationId,
    /// The installation's page on GitHub, where its owner approves a new permission.
    pub installation_url: Url,
    pub permission: GitHubInstallationPermission,
    pub id: GitHubRepositoryId,
    pub full_name: GitHubRepositoryName,
}

#[derive(Deserialize)]
struct JsonRefusal {
    message: String,
}

/// One page of installations or of repositories.
#[derive(Deserialize)]
struct JsonPage<T> {
    #[serde(alias = "installations", alias = "repositories")]
    items: Vec<T>,
}

#[derive(Deserialize)]
struct JsonInstallation {
    id: GitHubInstallationId,
    html_url: Url,
    suspended_at: Option<IgnoredAny>,
    permissions: JsonInstallationPermissions,
}

/// What the installation has granted the app.
#[derive(Deserialize)]
struct JsonInstallationPermissions {
    contents: Option<JsonAccess>,
}

#[derive(PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum JsonAccess {
    Write,
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct JsonRepository {
    id: GitHubRepositoryId,
    owner: JsonOwner,
    name: String,
    permissions: JsonRepositoryPermissions,
}

#[derive(Deserialize)]
pub(crate) struct JsonOwner {
    pub login: String,
}

/// The user's own access to the repository.
#[derive(Deserialize)]
struct JsonRepositoryPermissions {
    push: bool,
}

#[cfg(test)]
mod tests {
    use bencher_valid::{GitHubInstallationId, GitHubRepositoryId};
    use oauth2::AccessToken;

    use super::{
        GitHubApi, GitHubApiError, GitHubInstallationPermission, GitHubRepository, PER_PAGE,
    };
    use crate::test_server::{Received, Server};

    const USER_TOKEN: &str = "user-token-marker";

    fn installation(id: u64, suspended_at: &str, permissions: &str) -> String {
        format!(
            r#"{{"id": {id}, "html_url": "{}", "suspended_at": {suspended_at}, "permissions": {permissions}}}"#,
            installation_url(id)
        )
    }

    fn installation_url(id: u64) -> String {
        format!("https://github.com/settings/installations/{id}")
    }

    fn repository(id: u64, push: bool) -> String {
        named_repository(id, &format!("repo-{id}"), push)
    }

    fn named_repository(id: u64, name: &str, push: bool) -> String {
        format!(
            r#"{{"id": {id}, "name": "{name}", "full_name": "octo-cat/{name}", "owner": {{"login": "octo-cat", "id": 1}}, "permissions": {{"admin": false, "push": {push}, "pull": true}}}}"#
        )
    }

    fn page(key: &str, items: &[String]) -> (u16, String) {
        (
            200,
            format!(r#"{{"total_count": 0, "{key}": [{}]}}"#, items.join(", ")),
        )
    }

    fn pushable(
        installation: u64,
        permission: GitHubInstallationPermission,
        id: u64,
    ) -> GitHubRepository {
        GitHubRepository {
            installation: GitHubInstallationId::try_from(installation).unwrap(),
            installation_url: installation_url(installation).parse().unwrap(),
            permission,
            id: GitHubRepositoryId::try_from(id).unwrap(),
            full_name: format!("octo-cat/repo-{id}").parse().unwrap(),
        }
    }

    // Kills dropping the push filter or the suspended filter, attaching the wrong installation or
    // the wrong installation's page,
    // stopping before a full page's successor or after a short page, granting contents write to
    // an installation with any other access, parsing the name of a repository without push, and
    // building a URL that misses the base's path.
    #[tokio::test]
    async fn lists_only_repositories_with_push_on_every_page() {
        // Repository 1 through 100 fill the first page, and every third one is read only.
        let first: Vec<String> = (1..=100).map(|id| repository(id, id % 3 != 0)).collect();
        assert_eq!(first.len(), PER_PAGE);
        let server = Server::start(vec![
            page(
                "installations",
                &[
                    installation(11, "null", r#"{"contents": "write", "metadata": "read"}"#),
                    installation(
                        33,
                        r#""2024-07-11T06:23:00Z""#,
                        r#"{"contents": "write", "metadata": "read"}"#,
                    ),
                    installation(22, "null", r#"{"contents": "read", "metadata": "read"}"#),
                    installation(44, "null", r#"{"metadata": "read"}"#),
                    installation(55, "null", r#"{"contents": "admin", "metadata": "read"}"#),
                ],
            ),
            page("repositories", &first),
            page("repositories", &[repository(101, true)]),
            page(
                "repositories",
                &[
                    repository(201, false),
                    repository(202, true),
                    named_repository(203, "..", false),
                ],
            ),
            page("repositories", &[repository(401, true)]),
            page("repositories", &[repository(501, true)]),
        ])
        .await;
        let api = GitHubApi::new(server.base()).unwrap();

        let repositories = api
            .user_repositories(&AccessToken::new(USER_TOKEN.to_owned()))
            .await
            .unwrap();

        let expected: Vec<GitHubRepository> = (1..=100)
            .filter(|id| id % 3 != 0)
            .chain([101])
            .map(|id| pushable(11, GitHubInstallationPermission::Granted, id))
            .chain([
                pushable(22, GitHubInstallationPermission::ApprovalRequired, 202),
                pushable(44, GitHubInstallationPermission::ApprovalRequired, 401),
                pushable(55, GitHubInstallationPermission::ApprovalRequired, 501),
            ])
            .collect();
        assert_eq!(repositories, expected);
        let received = server.received().await;
        let request_lines: Vec<&str> = received.iter().map(Received::request_line).collect();
        assert_eq!(
            request_lines,
            [
                "GET /github/user/installations?per_page=100&page=1 HTTP/1.1",
                "GET /github/user/installations/11/repositories?per_page=100&page=1 HTTP/1.1",
                "GET /github/user/installations/11/repositories?per_page=100&page=2 HTTP/1.1",
                "GET /github/user/installations/22/repositories?per_page=100&page=1 HTTP/1.1",
                "GET /github/user/installations/44/repositories?per_page=100&page=1 HTTP/1.1",
                "GET /github/user/installations/55/repositories?per_page=100&page=1 HTTP/1.1",
            ]
        );
        for request in &received {
            assert_eq!(
                request.header("authorization"),
                Some(format!("Bearer {USER_TOKEN}").as_str())
            );
        }
    }

    // Kills skipping, or passing through, a pushable repository whose name is not two path
    // segments.
    #[tokio::test]
    async fn a_repository_name_that_does_not_parse_fails_the_listing() {
        let server = Server::start(vec![
            page(
                "installations",
                &[installation(11, "null", r#"{"contents": "write"}"#)],
            ),
            page(
                "repositories",
                &[repository(1, true), named_repository(2, "..", true)],
            ),
        ])
        .await;
        let api = GitHubApi::new(server.base()).unwrap();

        let error = api
            .user_repositories(&AccessToken::new(USER_TOKEN.to_owned()))
            .await
            .unwrap_err();

        assert!(
            matches!(error, GitHubApiError::RepositoryName(_)),
            "{error:?}"
        );
        server.received().await;
    }
}
