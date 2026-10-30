#![cfg(feature = "plus")]

use std::{net::IpAddr, sync::Arc, time::Duration};

use async_trait::async_trait;
use bencher_valid::{NonEmpty, Secret};
use slog::Logger;
use tokio::time::error::Elapsed;
use url::Url;

const SITEVERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";
/// How long a check waits for Cloudflare before it refuses.
const TIMEOUT: Duration = Duration::from_secs(10);

/// Cloudflare Turnstile, which checks that a person, not a script, asked: it verifies the token
/// a widget on the console issued, and refuses whenever it cannot.
pub struct Turnstile {
    secret: Secret,
    /// The console's host, where every token must come from.
    hostname: String,
    site_verify: Arc<dyn SiteVerify>,
}

#[derive(Debug, thiserror::Error)]
pub enum TurnstileError {
    #[error("No Turnstile token")]
    MissingToken,
    #[error("Failed to send the Turnstile verification request: {0}")]
    Send(reqwest::Error),
    #[error("Turnstile verification answered {0}")]
    Status(reqwest::StatusCode),
    #[error("Failed to read the Turnstile verification response: {0}")]
    Read(reqwest::Error),
    #[error("Turnstile verification did not answer in time: {0}")]
    Timeout(Elapsed),
    #[error("Failed to parse the Turnstile verification response: {0}")]
    Parse(serde_json::Error),
    #[error("Turnstile refused the token: {0:?}")]
    Refused(Vec<String>),
    #[error("Turnstile token is for {got:?}, not {expected}")]
    Hostname {
        expected: String,
        got: Option<String>,
    },
}

impl Turnstile {
    /// Verify tokens with Cloudflare.
    pub fn new(secret: Secret, console_url: &Url) -> Self {
        Self::with_site_verify(secret, console_url, Arc::new(Cloudflare::default()))
    }

    /// Verify tokens with `site_verify` in place of Cloudflare.
    pub fn with_site_verify(
        secret: Secret,
        console_url: &Url,
        site_verify: Arc<dyn SiteVerify>,
    ) -> Self {
        Self {
            secret,
            hostname: console_url.host_str().unwrap_or_default().to_owned(),
            site_verify,
        }
    }

    /// Verify the token from the reader at `remote_ip`: it must be present, Cloudflare must
    /// answer in time that it succeeded, and it must come from the console's host.
    pub async fn verify(
        &self,
        log: &Logger,
        token: Option<&NonEmpty>,
        remote_ip: Option<IpAddr>,
    ) -> Result<(), TurnstileError> {
        self.verify_inner(token, remote_ip)
            .await
            .inspect_err(|err| slog::warn!(log, "Turnstile verification failed"; "error" => %err))
    }

    async fn verify_inner(
        &self,
        token: Option<&NonEmpty>,
        remote_ip: Option<IpAddr>,
    ) -> Result<(), TurnstileError> {
        let token = token.ok_or(TurnstileError::MissingToken)?;
        let form = SiteVerifyForm {
            secret: self.secret.clone(),
            response: token.clone(),
            remote_ip,
        };
        let body = tokio::time::timeout(TIMEOUT, self.site_verify.post(&form))
            .await
            .map_err(TurnstileError::Timeout)??;
        let SiteVerifyResponse {
            success,
            hostname,
            error_codes,
        } = serde_json::from_str(&body).map_err(TurnstileError::Parse)?;
        if !success {
            return Err(TurnstileError::Refused(error_codes));
        }
        if hostname.as_deref() != Some(self.hostname.as_str()) {
            return Err(TurnstileError::Hostname {
                expected: self.hostname.clone(),
                got: hostname,
            });
        }
        Ok(())
    }
}

/// Cloudflare's siteverify endpoint, or a stand-in for it.
#[async_trait]
pub trait SiteVerify: Send + Sync {
    /// Post the form and return the body of a successful response.
    async fn post(&self, form: &SiteVerifyForm) -> Result<String, TurnstileError>;
}

// https://developers.cloudflare.com/turnstile/get-started/server-side-validation/
#[derive(serde::Serialize)]
pub struct SiteVerifyForm {
    secret: Secret,
    response: NonEmpty,
    #[serde(rename = "remoteip", skip_serializing_if = "Option::is_none")]
    remote_ip: Option<IpAddr>,
}

impl SiteVerifyForm {
    /// The reader's address, as the check sends it to Cloudflare.
    pub fn remote_ip(&self) -> Option<IpAddr> {
        self.remote_ip
    }
}

#[derive(Debug, serde::Deserialize)]
struct SiteVerifyResponse {
    success: bool,
    hostname: Option<String>,
    #[serde(default, rename = "error-codes")]
    error_codes: Vec<String>,
}

#[derive(Default)]
struct Cloudflare(reqwest::Client);

#[async_trait]
impl SiteVerify for Cloudflare {
    async fn post(&self, form: &SiteVerifyForm) -> Result<String, TurnstileError> {
        self.0
            .post(SITEVERIFY_URL)
            .timeout(TIMEOUT)
            .form(form)
            .send()
            .await
            .map_err(TurnstileError::Send)?
            .error_for_status()
            .map_err(|e| {
                e.status()
                    .map_or(TurnstileError::Send(e), TurnstileError::Status)
            })?
            .text()
            .await
            .map_err(TurnstileError::Read)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future,
        net::IpAddr,
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use bencher_valid::{NonEmpty, Secret};
    use pretty_assertions::assert_eq;
    use url::Url;

    use super::{SiteVerify, SiteVerifyForm, Turnstile, TurnstileError};

    /// Answers every check the same way, and keeps the address each was for.
    struct Answer {
        answer: Box<dyn Fn() -> Result<String, TurnstileError> + Send + Sync>,
        remote_ips: Mutex<Vec<Option<IpAddr>>>,
    }

    #[async_trait]
    impl SiteVerify for Answer {
        async fn post(&self, form: &SiteVerifyForm) -> Result<String, TurnstileError> {
            self.remote_ips.lock().unwrap().push(form.remote_ip());
            (self.answer)()
        }
    }

    /// Never answers.
    struct Silent;

    #[async_trait]
    impl SiteVerify for Silent {
        async fn post(&self, _form: &SiteVerifyForm) -> Result<String, TurnstileError> {
            future::pending().await
        }
    }

    fn turnstile(site_verify: Arc<dyn SiteVerify>) -> Turnstile {
        let console_url: Url = "https://bencher.dev".parse().unwrap();
        let secret: Secret = "turnstile-secret".parse().unwrap();
        Turnstile::with_site_verify(secret, &console_url, site_verify)
    }

    fn answer(
        answer: impl Fn() -> Result<String, TurnstileError> + Send + Sync + 'static,
    ) -> Arc<Answer> {
        Arc::new(Answer {
            answer: Box::new(answer),
            remote_ips: Mutex::new(Vec::new()),
        })
    }

    fn body(body: &'static str) -> Arc<Answer> {
        answer(move || Ok(body.to_owned()))
    }

    async fn verify(turnstile: &Turnstile) -> Result<(), TurnstileError> {
        let token: NonEmpty = "a-turnstile-token".parse().unwrap();
        turnstile
            .verify_inner(Some(&token), Some([203, 0, 113, 7].into()))
            .await
    }

    #[tokio::test]
    async fn verify_passes_a_success_from_the_console_host() {
        let site_verify = body(r#"{"success":true,"hostname":"bencher.dev","error-codes":[]}"#);
        verify(&turnstile(site_verify.clone())).await.unwrap();
        assert_eq!(
            *site_verify.remote_ips.lock().unwrap(),
            [Some([203, 0, 113, 7].into())]
        );
    }

    #[tokio::test]
    async fn verify_refuses_a_missing_token() {
        let site_verify = body(r#"{"success":true,"hostname":"bencher.dev"}"#);
        let turnstile = turnstile(site_verify.clone());
        assert!(matches!(
            turnstile.verify_inner(None, None).await,
            Err(TurnstileError::MissingToken)
        ));
        assert!(site_verify.remote_ips.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn verify_refuses_a_failure() {
        let site_verify = body(
            r#"{"success":false,"hostname":"bencher.dev","error-codes":["invalid-input-response"]}"#,
        );
        assert!(matches!(
            verify(&turnstile(site_verify)).await,
            Err(TurnstileError::Refused(codes)) if codes == ["invalid-input-response"]
        ));
    }

    #[tokio::test]
    async fn verify_refuses_another_host() {
        for answer in [
            r#"{"success":true,"hostname":"evil.example"}"#,
            r#"{"success":true,"hostname":"bencher.dev.evil.example"}"#,
            r#"{"success":true}"#,
        ] {
            assert!(
                matches!(
                    verify(&turnstile(body(answer))).await,
                    Err(TurnstileError::Hostname { .. })
                ),
                "{answer}"
            );
        }
    }

    #[tokio::test]
    async fn verify_refuses_an_error() {
        let site_verify = answer(|| {
            Err(TurnstileError::Status(
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            ))
        });
        assert!(matches!(
            verify(&turnstile(site_verify)).await,
            Err(TurnstileError::Status(_))
        ));
    }

    #[tokio::test]
    async fn verify_refuses_an_answer_it_cannot_parse() {
        for answer in [
            "",
            "<html>",
            r#"{"hostname":"bencher.dev"}"#,
            r#"{"success":"true","hostname":"bencher.dev"}"#,
        ] {
            assert!(
                matches!(
                    verify(&turnstile(body(answer))).await,
                    Err(TurnstileError::Parse(_))
                ),
                "{answer}"
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn verify_refuses_no_answer() {
        assert!(matches!(
            verify(&turnstile(Arc::new(Silent))).await,
            Err(TurnstileError::Timeout(_))
        ));
    }
}
