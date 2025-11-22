use std::{net::Ipv4Addr, sync::LazyLock};

use bencher_valid::{NonEmpty, Secret, Url};
use chrono::{DateTime, Utc};

#[expect(clippy::expect_used)]
static VERIFY_URL: LazyLock<Url> = LazyLock::new(|| {
    "https://www.google.com/recaptcha/api/siteverify"
        .parse()
        .expect("valid recaptcha verify url")
});

pub struct RecaptchaClient {
    secret: Secret,
    client: reqwest::Client,
}

impl RecaptchaClient {
    pub fn new(secret: Secret) -> Self {
        Self {
            secret,
            client: reqwest::Client::new(),
        }
    }

    pub async fn verify(&self, response_token: &NonEmpty, remote_ip: Option<Ipv4Addr>) {}
}

// https://developers.google.com/recaptcha/docs/verify#api_request
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
struct RecaptchaBody {
    /// The shared key between your site and reCAPTCHA.
    pub secret: Secret,
    /// The user response token provided by the reCAPTCHA client-side integration on your site.
    pub token: NonEmpty,
    /// The user's IP address.
    #[serde(rename = "remoteip", skip_serializing_if = "Option::is_none")]
    pub remote_ip: Option<Ipv4Addr>,
}

// https://developers.google.com/recaptcha/docs/v3#site_verify_response
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
struct RecaptchaResponse {
    /// Whether this request was a valid reCAPTCHA token for your site.
    pub success: bool,
    /// The score for this request (0.0 - 1.0)
    pub score: f32,
    // The action name for this request (important to verify)
    pub action: String,
    /// The timestamp of the challenge load (ISO format yyyy-MM-dd'T'HH:mm:ssZZ).
    pub challenge_ts: DateTime<Utc>,
    /// The hostname of the site where the reCAPTCHA was solved.
    pub hostname: NonEmpty,
    /// Optional error codes.
    #[serde(rename = "error-codes")]
    pub error_codes: Option<Vec<ErrorCode>>,
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
    // Future-proof for unknown codes
    #[serde(other)]
    Other,
}
