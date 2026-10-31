use aws_lc_rs::hmac;
use bencher_valid::{GitHubInstallationId, GitHubRepositoryId, Secret};
use serde::Deserialize;

const SIGNATURE_PREFIX: &str = "sha256=";

/// What a webhook delivery changed about an installation of the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHubInstallationChange {
    /// The app was uninstalled.
    Uninstalled(GitHubInstallationId),
    Suspended(GitHubInstallationId),
    /// The installation can mint again, with the repositories the delivery names, if any.
    Unsuspended {
        installation: GitHubInstallationId,
        repositories: Vec<GitHubRepositoryId>,
    },
    /// The repositories left the installation.
    Removed {
        installation: GitHubInstallationId,
        repositories: Vec<GitHubRepositoryId>,
    },
    /// The repositories are in the installation, newly or again.
    Added {
        installation: GitHubInstallationId,
        repositories: Vec<GitHubRepositoryId>,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum GitHubWebhookError {
    #[error("The webhook delivery's signature does not match its body")]
    Signature,
    #[error("Failed to parse a webhook delivery: {0}")]
    Json(serde_json::Error),
}

/// The change a webhook delivery makes, once its signature proves that `secret` signed its body.
/// Any other event or action makes none.
pub(crate) fn delivery(
    secret: &Secret,
    signature: Option<&str>,
    event: Option<&str>,
    body: &[u8],
) -> Result<Option<GitHubInstallationChange>, GitHubWebhookError> {
    verify(secret, signature, body)?;
    match event {
        Some("installation") => {
            serde_json::from_slice(body).map(JsonInstallationEvent::into_change)
        },
        Some("installation_repositories") => {
            serde_json::from_slice(body).map(JsonInstallationRepositoriesEvent::into_change)
        },
        _ => Ok(None),
    }
    .map_err(GitHubWebhookError::Json)
}

fn verify(secret: &Secret, signature: Option<&str>, body: &[u8]) -> Result<(), GitHubWebhookError> {
    let tag = signature
        .and_then(|signature| signature.strip_prefix(SIGNATURE_PREFIX))
        .and_then(|digest| hex::decode(digest).ok())
        .ok_or(GitHubWebhookError::Signature)?;
    let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_ref().as_bytes());
    // `verify` compares the tags in constant time.
    hmac::verify(&key, body, &tag).map_err(|_unspecified| GitHubWebhookError::Signature)
}

/// An `installation` delivery, by the actions that change a link.
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum JsonInstallationEvent {
    Created {
        installation: JsonInstallation,
        #[serde(default)]
        repositories: Vec<JsonRepository>,
    },
    Deleted {
        installation: JsonInstallation,
    },
    Suspend {
        installation: JsonInstallation,
    },
    Unsuspend {
        installation: JsonInstallation,
        #[serde(default)]
        repositories: Vec<JsonRepository>,
    },
    #[serde(other)]
    Other,
}

impl JsonInstallationEvent {
    fn into_change(self) -> Option<GitHubInstallationChange> {
        Some(match self {
            Self::Created {
                installation,
                repositories,
            } => GitHubInstallationChange::Added {
                installation: installation.id,
                repositories: ids(repositories),
            },
            Self::Deleted { installation } => {
                GitHubInstallationChange::Uninstalled(installation.id)
            },
            Self::Suspend { installation } => GitHubInstallationChange::Suspended(installation.id),
            Self::Unsuspend {
                installation,
                repositories,
            } => GitHubInstallationChange::Unsuspended {
                installation: installation.id,
                repositories: ids(repositories),
            },
            Self::Other => return None,
        })
    }
}

/// An `installation_repositories` delivery, by the actions that change a link.
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum JsonInstallationRepositoriesEvent {
    Added {
        installation: JsonInstallation,
        repositories_added: Vec<JsonRepository>,
    },
    Removed {
        installation: JsonInstallation,
        repositories_removed: Vec<JsonRepository>,
    },
    #[serde(other)]
    Other,
}

impl JsonInstallationRepositoriesEvent {
    fn into_change(self) -> Option<GitHubInstallationChange> {
        Some(match self {
            Self::Added {
                installation,
                repositories_added,
            } => GitHubInstallationChange::Added {
                installation: installation.id,
                repositories: ids(repositories_added),
            },
            Self::Removed {
                installation,
                repositories_removed,
            } => GitHubInstallationChange::Removed {
                installation: installation.id,
                repositories: ids(repositories_removed),
            },
            Self::Other => return None,
        })
    }
}

fn ids(repositories: Vec<JsonRepository>) -> Vec<GitHubRepositoryId> {
    repositories
        .into_iter()
        .map(|repository| repository.id)
        .collect()
}

#[derive(Deserialize)]
struct JsonInstallation {
    id: GitHubInstallationId,
}

#[derive(Deserialize)]
struct JsonRepository {
    id: GitHubRepositoryId,
}

#[cfg(test)]
mod tests {
    use aws_lc_rs::hmac;
    use bencher_valid::{GitHubInstallationId, GitHubRepositoryId, Secret};

    use super::{GitHubInstallationChange, GitHubWebhookError, delivery};

    const WEBHOOK_SECRET: &str = "webhook-secret-marker";

    fn secret() -> Secret {
        WEBHOOK_SECRET.parse().unwrap()
    }

    fn sign(secret: &str, body: &str) -> String {
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
        format!(
            "sha256={}",
            hex::encode(hmac::sign(&key, body.as_bytes()).as_ref())
        )
    }

    fn installation(id: u64) -> GitHubInstallationId {
        GitHubInstallationId::try_from(id).unwrap()
    }

    fn repositories(ids: &[u64]) -> Vec<GitHubRepositoryId> {
        ids.iter()
            .map(|id| GitHubRepositoryId::try_from(*id).unwrap())
            .collect()
    }

    fn repository_list(ids: &[u64]) -> String {
        let repositories: Vec<String> = ids
            .iter()
            .map(|id| {
                format!(
                    r#"{{"id": {id}, "node_id": "R_{id}", "name": "repo-{id}", "full_name": "octo-cat/repo-{id}", "private": false}}"#
                )
            })
            .collect();
        format!("[{}]", repositories.join(", "))
    }

    fn installation_event(action: &str, repositories: Option<&[u64]>) -> String {
        let repositories = repositories
            .map(|ids| format!(r#""repositories": {}, "#, repository_list(ids)))
            .unwrap_or_default();
        format!(
            r#"{{"action": "{action}", "installation": {{"id": 2048, "account": {{"login": "octo-cat"}}, "repository_selection": "selected", "suspended_at": null}}, {repositories}"requester": null, "sender": {{"login": "octo-cat", "id": 1}}}}"#
        )
    }

    fn repositories_event(action: &str, added: &[u64], removed: &[u64]) -> String {
        format!(
            r#"{{"action": "{action}", "installation": {{"id": 2048}}, "repository_selection": "selected", "repositories_added": {}, "repositories_removed": {}, "requester": null, "sender": {{"login": "octo-cat", "id": 1}}}}"#,
            repository_list(added),
            repository_list(removed)
        )
    }

    // GitHub's own example: kills any other algorithm, key, or digest encoding.
    #[test]
    fn a_delivery_signed_as_github_signs_is_accepted() {
        let secret = "It's a Secret to Everybody".parse().unwrap();
        let signature = "sha256=757107ea0eb2509fc211221cce984b8a37570b6d7586c22c46f4379c8b043e17";

        let change = delivery(&secret, Some(signature), None, b"Hello, World!").unwrap();

        assert_eq!(change, None);
    }

    // The body is not JSON, so this kills parsing it before the signature is checked, and
    // accepting a delivery with no signature, a signature by any other secret or over any other
    // body, a truncated one, or one without its prefix.
    #[test]
    fn a_delivery_without_a_good_signature_is_refused_before_it_is_read() {
        let body = "not-json";
        let good = sign(WEBHOOK_SECRET, body);
        let truncated = good.get(..good.len() - 2).unwrap().to_owned();
        let unprefixed = good.strip_prefix("sha256=").unwrap().to_owned();
        for signature in [
            None,
            Some(String::new()),
            Some(sign("another-secret", body)),
            Some(sign(WEBHOOK_SECRET, "another-body")),
            Some(truncated),
            Some(unprefixed),
            Some(good.replace("sha256=", "sha1=")),
            Some("sha256=not-hex".to_owned()),
        ] {
            let error = delivery(
                &secret(),
                signature.as_deref(),
                Some("installation"),
                body.as_bytes(),
            )
            .unwrap_err();

            assert!(
                matches!(error, GitHubWebhookError::Signature),
                "{signature:?}: {error:?}"
            );
        }
        let error = delivery(
            &secret(),
            Some(&good),
            Some("installation"),
            body.as_bytes(),
        )
        .unwrap_err();
        assert!(matches!(error, GitHubWebhookError::Json(_)), "{error:?}");
    }

    // Kills reading an action as any other change, dropping or swapping the repositories it names,
    // and failing an event or an action that changes no link.
    #[test]
    fn each_delivery_reads_as_its_change() {
        let installation = installation(2048);
        for (event, body, expected) in [
            (
                "installation",
                installation_event("created", Some(&[1, 2])),
                Some(GitHubInstallationChange::Added {
                    installation,
                    repositories: repositories(&[1, 2]),
                }),
            ),
            (
                "installation",
                installation_event("created", None),
                Some(GitHubInstallationChange::Added {
                    installation,
                    repositories: Vec::new(),
                }),
            ),
            (
                "installation",
                installation_event("deleted", Some(&[1])),
                Some(GitHubInstallationChange::Uninstalled(installation)),
            ),
            (
                "installation",
                installation_event("suspend", None),
                Some(GitHubInstallationChange::Suspended(installation)),
            ),
            (
                "installation",
                installation_event("unsuspend", None),
                Some(GitHubInstallationChange::Unsuspended {
                    installation,
                    repositories: Vec::new(),
                }),
            ),
            (
                "installation",
                installation_event("unsuspend", Some(&[3])),
                Some(GitHubInstallationChange::Unsuspended {
                    installation,
                    repositories: repositories(&[3]),
                }),
            ),
            (
                "installation_repositories",
                repositories_event("added", &[4, 5], &[6]),
                Some(GitHubInstallationChange::Added {
                    installation,
                    repositories: repositories(&[4, 5]),
                }),
            ),
            (
                "installation_repositories",
                repositories_event("removed", &[4], &[6, 7]),
                Some(GitHubInstallationChange::Removed {
                    installation,
                    repositories: repositories(&[6, 7]),
                }),
            ),
            (
                "installation",
                installation_event("new_permissions_accepted", None),
                None,
            ),
            (
                "installation_repositories",
                r#"{"action": "renamed", "sender": {"login": "octo-cat"}}"#.to_owned(),
                None,
            ),
            ("push", r#"{"ref": "refs/heads/main"}"#.to_owned(), None),
            (
                "ping",
                r#"{"zen": "Keep it logically awesome."}"#.to_owned(),
                None,
            ),
        ] {
            let signature = sign(WEBHOOK_SECRET, &body);

            let change =
                delivery(&secret(), Some(&signature), Some(event), body.as_bytes()).unwrap();

            assert_eq!(change, expected, "{event}: {body}");
        }
    }
}
