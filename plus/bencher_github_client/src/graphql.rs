use std::{collections::HashMap, fmt::Write as _};

use bencher_valid::{GitHash, GitHubRepositoryName, Secret};
use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};

use crate::api::{GitHubApi, GitHubApiError, SECONDARY_RATE_LIMIT};

/// The type GitHub gives the error of a GraphQL answer that the rate limit stopped.
const RATE_LIMITED: &str = "RATE_LIMITED";

#[derive(Debug, thiserror::Error)]
pub enum GitHubCommitsError {
    #[error("Failed to ask GitHub for commits: {0}")]
    Api(GitHubApiError),
    /// A 200 whose errors say that a rate limit stopped it, with its rate-limit headers.
    #[error("GitHub rate limited a request for commits")]
    RateLimited(HeaderMap),
    /// A 200 with errors, by their types, or with no repository or no answer for a hash.
    #[error("GitHub did not answer for every commit: {0:?}")]
    Unanswered(Vec<String>),
}

/// One request for the message of every hash, each by its own alias.
pub(crate) async fn commit_messages(
    api: &GitHubApi,
    token: &Secret,
    full_name: &GitHubRepositoryName,
    hashes: &[GitHash],
) -> Result<Vec<(GitHash, Option<String>)>, GitHubCommitsError> {
    let request = JsonGraphQlRequest {
        query: query(hashes),
        variables: JsonVariables {
            owner: full_name.owner(),
            name: full_name.name(),
        },
    };
    let (JsonGraphQlAnswer { data, errors }, headers) = api
        .post_counted::<_, JsonGraphQlAnswer>(&["graphql"], token.as_ref(), &request)
        .await
        .map_err(GitHubCommitsError::Api)?;
    if !errors.is_empty() {
        let secondary = errors.iter().any(|JsonGraphQlError { message, .. }| {
            message.to_lowercase().contains(SECONDARY_RATE_LIMIT)
        });
        let types = errors
            .into_iter()
            .map(|JsonGraphQlError { kind, .. }| kind.unwrap_or_default())
            .collect::<Vec<_>>();
        return Err(if secondary || rate_limited(&types, &headers) {
            GitHubCommitsError::RateLimited(headers)
        } else {
            GitHubCommitsError::Unanswered(types)
        });
    }
    let Some(JsonData {
        repository: Some(mut objects),
    }) = data
    else {
        return Err(GitHubCommitsError::Unanswered(Vec::new()));
    };
    hashes
        .iter()
        .enumerate()
        .map(|(index, hash)| {
            // A missing object is null, and an object that is no commit has no message.
            let object = objects
                .remove(&alias(index))
                .ok_or_else(|| GitHubCommitsError::Unanswered(Vec::new()))?;
            Ok((hash.clone(), object.and_then(|object| object.message)))
        })
        .collect()
}

/// The aliased lookups of every hash in the one repository. A hash is hexadecimal, so it goes
/// into the query as it is.
fn query(hashes: &[GitHash]) -> String {
    let mut query = String::from(
        "query($owner: String!, $name: String!) { repository(owner: $owner, name: $name) {",
    );
    for (index, hash) in hashes.iter().enumerate() {
        let _infallible = write!(
            query,
            " {}: object(oid: \"{}\") {{ ... on Commit {{ message }} }}",
            alias(index),
            hash.as_ref().to_ascii_lowercase()
        );
    }
    query.push_str(" } }");
    query
}

fn alias(index: usize) -> String {
    format!("c{index}")
}

/// A rate limit error, or errors that come with a spent or paused rate limit.
fn rate_limited(types: &[String], headers: &HeaderMap) -> bool {
    types.iter().any(|kind| kind == RATE_LIMITED)
        || headers.contains_key("retry-after")
        || headers
            .get("x-ratelimit-remaining")
            .is_some_and(|remaining| remaining == "0")
}

#[derive(Serialize)]
struct JsonGraphQlRequest<'a> {
    query: String,
    variables: JsonVariables<'a>,
}

#[derive(Serialize)]
struct JsonVariables<'a> {
    owner: &'a str,
    name: &'a str,
}

#[derive(Deserialize)]
struct JsonGraphQlAnswer {
    data: Option<JsonData>,
    #[serde(default)]
    errors: Vec<JsonGraphQlError>,
}

#[derive(Deserialize)]
struct JsonData {
    repository: Option<HashMap<String, Option<JsonObject>>>,
}

#[derive(Deserialize)]
struct JsonObject {
    message: Option<String>,
}

#[derive(Deserialize)]
struct JsonGraphQlError {
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    message: String,
}
