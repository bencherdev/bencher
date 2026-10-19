use std::{collections::HashMap, fmt, sync::Arc};

use bencher_egress::{EgressClient, GetJsonError};
use bencher_json::{
    Clock, OAuthClientId, OAuthClientMetadata, OAuthClientMetadataError, OAuthRedirectUri,
    system::config::JsonOAuthServerClient,
};

use crate::documents::Documents;

/// Hosts whose client IDs the consent page vouches for.
const VERIFIED_HOSTS: [&str; 2] = ["chatgpt.com", "claude.ai"];

/// The OAuth clients that may sign in, and the metadata each registered.
pub struct OAuthClients {
    /// The clients this server was configured with, each with its metadata unless that is to be
    /// fetched. Unset, any client with a valid metadata document may sign in.
    configured: Option<HashMap<OAuthClientId, Option<Arc<OAuthClientMetadata>>>>,
    documents: Documents,
}

/// Why a client cannot sign in. Every variant answers the client alike.
#[derive(Debug, thiserror::Error)]
pub enum OAuthClientError {
    #[error("Client is not one this server allows")]
    NotAllowed,
    #[error("Too many client metadata documents fetched for this address")]
    Throttled,
    #[error("Too many client metadata documents are being fetched")]
    Busy,
    #[error("Failed to fetch the client metadata document: {0}")]
    Fetch(GetJsonError),
    #[error(transparent)]
    Document(OAuthClientMetadataError),
}

impl OAuthClients {
    pub fn new(clients: Option<Vec<JsonOAuthServerClient>>) -> Self {
        Self::with_documents(clients, Documents::new())
    }

    /// Fetch metadata documents with `egress`.
    pub fn with_egress(clients: Option<Vec<JsonOAuthServerClient>>, egress: EgressClient) -> Self {
        Self::with_documents(clients, Documents::with_egress(egress))
    }

    fn with_documents(clients: Option<Vec<JsonOAuthServerClient>>, documents: Documents) -> Self {
        let configured = clients.map(|clients| {
            clients
                .into_iter()
                .filter_map(|client| {
                    let JsonOAuthServerClient { id, metadata } = client;
                    // Parsing the configuration validates the metadata, so only a client built
                    // some other way is dropped here.
                    let metadata = metadata
                        .map(|metadata| metadata.validate(id.url()))
                        .transpose()
                        .ok()?;
                    Some((id, metadata.map(Arc::new)))
                })
                .collect()
        });
        Self {
            configured,
            documents,
        }
    }

    /// The metadata a client registered, fetching its metadata document unless it is configured
    /// or cached. `charge` is called before a fetch, and may refuse it.
    pub async fn resolve<F>(
        &self,
        client_id: &OAuthClientId,
        clock: &Clock,
        charge: F,
    ) -> Result<Arc<OAuthClientMetadata>, OAuthClientError>
    where
        F: FnOnce() -> Result<(), Throttled>,
    {
        if let Some(configured) = &self.configured {
            let Some(metadata) = configured.get(client_id) else {
                return Err(OAuthClientError::NotAllowed);
            };
            if let Some(metadata) = metadata {
                return Ok(Arc::clone(metadata));
            }
        }
        self.documents.get(client_id.url(), clock, charge).await
    }

    /// Whether `client_id` may use its codes and tokens: any client, unless this server was
    /// configured with the only clients that may sign in.
    pub fn allows(&self, client_id: &OAuthClientId) -> bool {
        self.configured
            .as_ref()
            .is_none_or(|configured| configured.contains_key(client_id))
    }

    /// Whether the consent page vouches for a client answered at `redirect_uri`: one this server was
    /// configured with, or one on a well-known host that also receives the answer there or on the
    /// user's own computer, since any path on that host could serve a metadata document.
    pub fn is_verified(&self, client_id: &OAuthClientId, redirect_uri: &OAuthRedirectUri) -> bool {
        let host = client_id.host();
        let configured = self
            .configured
            .as_ref()
            .is_some_and(|configured| configured.contains_key(client_id));
        configured
            || (VERIFIED_HOSTS.contains(&host.as_str())
                && (redirect_uri.host() == host || redirect_uri.is_loopback()))
    }
}

/// The caller refused to pay for a metadata document fetch.
#[derive(Debug, Clone, Copy)]
pub struct Throttled;

impl fmt::Debug for OAuthClients {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuthClients")
            .field("configured", &self.configured)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io,
        net::{IpAddr, Ipv4Addr},
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use async_trait::async_trait;
    use bencher_egress::{EgressClient, EgressPolicy, EgressTimeouts, Lookup};
    use bencher_json::{
        Clock, DateTime, OAuthClientId, OAuthRedirectUri, system::config::JsonOAuthServerClient,
    };
    use serde_json::json;
    use tokio_rustls::rustls::crypto::aws_lc_rs;

    use super::{OAuthClientError, OAuthClients};

    const TIMEOUTS: EgressTimeouts = EgressTimeouts {
        connect: Duration::from_secs(5),
        total: Duration::from_secs(5),
    };

    /// Counts lookups and answers each with a private address, so no fetch ever connects.
    #[derive(Default)]
    struct PrivateLookup(AtomicUsize);

    #[async_trait]
    impl Lookup for PrivateLookup {
        async fn lookup(&self, _host: &str) -> io::Result<Vec<IpAddr>> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))])
        }
    }

    fn clients(configured: Option<serde_json::Value>, lookup: &Arc<PrivateLookup>) -> OAuthClients {
        let _provider = aws_lc_rs::default_provider().install_default();
        let configured = configured.map(|configured| {
            serde_json::from_value::<Vec<JsonOAuthServerClient>>(configured).unwrap()
        });
        let lookup: Arc<PrivateLookup> = Arc::clone(lookup);
        OAuthClients::with_egress(
            configured,
            EgressClient::with_lookup(EgressPolicy::Public, TIMEOUTS, lookup),
        )
    }

    fn client_id(client_id: &str) -> OAuthClientId {
        client_id.parse().unwrap()
    }

    fn clock() -> Clock {
        Clock::Custom(Arc::new(|| DateTime::TEST))
    }

    #[tokio::test]
    async fn configured_clients_are_the_only_clients() {
        let lookup = Arc::new(PrivateLookup::default());
        let clients = clients(
            Some(json!([
                {"id": "https://fetched.example/client.json"},
                {
                    "id": "https://inline.example/client.json",
                    "metadata": {
                        "client_id": "https://inline.example/client.json",
                        "client_name": "Inline",
                        "redirect_uris": ["http://127.0.0.1/callback"],
                    },
                },
            ])),
            &lookup,
        );
        let charges = AtomicUsize::new(0);
        let charge = || {
            charges.fetch_add(1, Ordering::SeqCst);
            Ok(())
        };

        let inline = clients
            .resolve(
                &client_id("https://inline.example/client.json"),
                &clock(),
                charge,
            )
            .await
            .unwrap();
        assert_eq!(inline.name.as_ref(), "Inline");
        assert_eq!(charges.load(Ordering::SeqCst), 0);

        let unknown = clients
            .resolve(
                &client_id("https://unknown.example/client.json"),
                &clock(),
                charge,
            )
            .await;
        assert!(
            matches!(unknown, Err(OAuthClientError::NotAllowed)),
            "{unknown:?}"
        );
        assert_eq!(charges.load(Ordering::SeqCst), 0);
        assert_eq!(lookup.0.load(Ordering::SeqCst), 0);

        let fetched = clients
            .resolve(
                &client_id("https://fetched.example/client.json"),
                &clock(),
                charge,
            )
            .await;
        assert!(
            matches!(fetched, Err(OAuthClientError::Fetch(_))),
            "{fetched:?}"
        );
        assert_eq!(charges.load(Ordering::SeqCst), 1);
        assert_eq!(lookup.0.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn without_configured_clients_any_client_is_fetched() {
        let lookup = Arc::new(PrivateLookup::default());
        let clients = clients(None, &lookup);
        let result = clients
            .resolve(
                &client_id("https://any.example/client.json"),
                &clock(),
                || Ok(()),
            )
            .await;
        assert!(
            matches!(result, Err(OAuthClientError::Fetch(_))),
            "{result:?}"
        );
        assert_eq!(lookup.0.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn verified_clients_are_configured_or_answered_on_a_well_known_host() {
        let lookup = Arc::new(PrivateLookup::default());
        let open = clients(None, &lookup);
        for (id, uri) in [
            (
                "https://chatgpt.com/oauth/client.json",
                "https://chatgpt.com/connector_platform_oauth_redirect",
            ),
            (
                "https://claude.ai/oauth/mcp-oauth-client-metadata",
                "https://claude.ai/api/mcp/auth_callback",
            ),
            (
                "https://claude.ai/oauth/claude-code-client-metadata",
                "http://localhost:53682/callback",
            ),
        ] {
            assert!(
                open.is_verified(&client_id(id), &redirect(uri)),
                "{id} {uri}"
            );
        }
        for (id, uri) in [
            (
                "https://chatgpt.com/user/content.json",
                "https://evil.example/cb",
            ),
            (
                "https://chatgpt.com/oauth/client.json",
                "https://evilchatgpt.com/cb",
            ),
            (
                "https://chatgpt.com/oauth/client.json",
                "https://claude.ai/api/mcp/auth_callback",
            ),
            (
                "https://chatgpt.com.evil.example/oauth/client.json",
                "https://chatgpt.com.evil.example/cb",
            ),
            (
                "https://evil.example/chatgpt.com/client.json",
                "https://evil.example/cb",
            ),
            (
                "https://www.claude.ai/oauth/client.json",
                "https://www.claude.ai/cb",
            ),
        ] {
            assert!(
                !open.is_verified(&client_id(id), &redirect(uri)),
                "{id} {uri}"
            );
        }

        let configured = clients(
            Some(json!([{"id": "https://client.example/client.json"}])),
            &lookup,
        );
        let elsewhere = redirect("https://evil.example/cb");
        assert!(
            configured.is_verified(&client_id("https://client.example/client.json"), &elsewhere)
        );
        assert!(
            !configured.is_verified(&client_id("https://other.example/client.json"), &elsewhere)
        );
    }

    fn redirect(redirect_uri: &str) -> OAuthRedirectUri {
        redirect_uri.parse().unwrap()
    }
}
