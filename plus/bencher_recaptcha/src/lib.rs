#![cfg(feature = "plus")]

use std::{net::IpAddr, sync::LazyLock};

use bencher_valid::{NonEmpty, RecaptchaAction, RecaptchaScore, Secret, Url};
use chrono::{DateTime, Utc};
use slog::Logger;

#[expect(clippy::expect_used)]
static VERIFY_URL: LazyLock<Url> = LazyLock::new(|| {
    "https://www.google.com/recaptcha/api/siteverify"
        .parse()
        .expect("valid recaptcha verify url")
});

pub struct RecaptchaClient {
    secret: Secret,
    min_score: RecaptchaScore,
    client: reqwest::Client,
}

#[derive(Debug, thiserror::Error)]
pub enum RecaptchaError {
    #[error("Failed to send reCAPTCHA verification request: {0}")]
    SendRequest(reqwest::Error),
    #[error("Failed to parse reCAPTCHA verification response: {0}")]
    ParseResponse(reqwest::Error),
    #[error("Failed to parse reCAPTCHA verification JSON: {0}")]
    ParseJson(serde_json::Error),

    #[error("reCAPTCHA verification failed: {0:?}")]
    VerificationFailed(Vec<ErrorCode>),
    #[error("{action} reCAPTCHA score too low ({score}) for {hostname} at {challenge_ts:?}")]
    ScoreTooLow {
        action: RecaptchaAction,
        score: RecaptchaScore,
        challenge_ts: DateTime<Utc>,
        hostname: NonEmpty,
    },
    #[error("reCAPTCHA action mismatch: expected {expected:?}, got {got:?}")]
    ActionMismatch {
        expected: RecaptchaAction,
        got: RecaptchaAction,
    },
}

impl RecaptchaClient {
    pub fn new(secret: Secret, min_score: RecaptchaScore) -> Self {
        Self {
            secret,
            min_score,
            client: reqwest::Client::new(),
        }
    }

    pub async fn verify(
        &self,
        log: &Logger,
        response_token: NonEmpty,
        recaptcha_action: RecaptchaAction,
        remote_ip: Option<IpAddr>,
    ) -> Result<(), RecaptchaError> {
        self.verify_inner(log, response_token, recaptcha_action, remote_ip)
            .await
            .inspect_err(|err| {
                slog::warn!(log, "reCAPTCHA verification failed"; "error" => %err);
            })
            .inspect_err(|_| {
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserRecaptchaFailure);
            })
    }

    async fn verify_inner(
        &self,
        log: &Logger,
        response_token: NonEmpty,
        recaptcha_action: RecaptchaAction,
        remote_ip: Option<IpAddr>,
    ) -> Result<(), RecaptchaError> {
        let body = RecaptchaBody {
            secret: self.secret.clone(),
            response: response_token,
            remote_ip,
        };

        let resp = self
            .client
            .post(VERIFY_URL.as_ref())
            .form(&body)
            .send()
            .await
            .map_err(RecaptchaError::SendRequest)?;

        let json_value: serde_json::Value =
            resp.json().await.map_err(RecaptchaError::ParseResponse)?;
        slog::info!(log, "reCAPTCHA verification response"; "response" => %json_value, "ip" => ?remote_ip);

        let recaptcha_response =
            serde_json::from_value(json_value.clone()).map_err(RecaptchaError::ParseJson)?;

        // todo(epompeii): Create a custom deserializer round `success` for better handling of the response
        match recaptcha_response {
            RecaptchaResponse::Ok(RecaptchaResponseOk {
                success,
                score,
                action,
                challenge_ts,
                hostname,
            }) => {
                if success {
                    if score < self.min_score {
                        Err(RecaptchaError::ScoreTooLow {
                            action,
                            score,
                            challenge_ts,
                            hostname,
                        })
                    } else if action != recaptcha_action {
                        Err(RecaptchaError::ActionMismatch {
                            expected: recaptcha_action,
                            got: action,
                        })
                    } else {
                        Ok(())
                    }
                } else {
                    debug_assert!(false, "RecaptchaResponse::Ok with success == false");
                    Ok(())
                }
            },
            RecaptchaResponse::Err(RecaptchaResponseErr {
                success,
                error_codes,
            }) => {
                if success {
                    debug_assert!(
                        false,
                        "RecaptchaResponse::Err with success == true: {error_codes:?}"
                    );
                    Ok(())
                } else {
                    Err(RecaptchaError::VerificationFailed(error_codes))
                }
            },
        }
    }
}

// https://developers.google.com/recaptcha/docs/verify#api_request
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
struct RecaptchaBody {
    /// The shared key between your site and reCAPTCHA.
    pub secret: Secret,
    /// The user response token provided by the reCAPTCHA client-side integration on your site.
    pub response: NonEmpty,
    /// The user's IP address.
    #[serde(rename = "remoteip", skip_serializing_if = "Option::is_none")]
    pub remote_ip: Option<IpAddr>,
}

// https://developers.google.com/recaptcha/docs/v3#site_verify_response
#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum RecaptchaResponse {
    Ok(RecaptchaResponseOk),
    Err(RecaptchaResponseErr),
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
struct RecaptchaResponseOk {
    /// Whether this request was a valid reCAPTCHA token for your site.
    pub success: bool,
    /// The score for this request (0.0 - 1.0)
    pub score: RecaptchaScore,
    // The action name for this request (important to verify)
    pub action: RecaptchaAction,
    /// The timestamp of the challenge load (ISO format yyyy-MM-dd'T'HH:mm:ssZZ).
    pub challenge_ts: DateTime<Utc>,
    /// The hostname of the site where the reCAPTCHA was solved.
    pub hostname: NonEmpty,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
struct RecaptchaResponseErr {
    /// Whether this request was a valid reCAPTCHA token for your site.
    pub success: bool,
    /// Optional error codes.
    #[serde(default, rename = "error-codes")]
    pub error_codes: Vec<ErrorCode>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorCode {
    // The secret parameter is missing.
    MissingInputSecret,
    // The secret parameter is invalid or malformed.
    InvalidInputSecret,
    // The response parameter is missing.
    MissingInputResponse,
    // The response parameter is invalid or malformed.
    InvalidInputResponse,
    // The request is invalid or malformed.
    BadRequest,
    // The response is no longer valid: either is too old or has been used previously.
    TimeoutOrDuplicate,
    // The domain is not in the list of allowed domains for the site key.
    BrowserError,
    // Future-proof for unknown codes
    #[serde(other)]
    Other,
}
