#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("Failed to encode JSON Web Token: {error}")]
    Encode { error: jsonwebtoken::errors::Error },
    #[error("Failed to decode JSON Web Token: {error}")]
    Decode { error: jsonwebtoken::errors::Error },
    #[error("Failed to parse JSON Web Token: {0}")]
    Parse(bencher_json::ValidError),
    #[error("Expired JSON Web Token ({exp} < {now}): {error}")]
    Expired {
        exp: i64,
        now: i64,
        error: jsonwebtoken::errors::Error,
    },
    #[error("Invalid organizational invite: {error}")]
    Invite { error: jsonwebtoken::errors::Error },
    #[error("Failed to extract OAuth state: {error}")]
    OAuthState { error: jsonwebtoken::errors::Error },
    #[error("Invalid public OCI token: {error}")]
    OciPublic { error: jsonwebtoken::errors::Error },
    #[error("Invalid authenticated OCI token: {error}")]
    OciAuth { error: jsonwebtoken::errors::Error },
    #[error("Invalid runner OCI token: {error}")]
    OciRunner { error: jsonwebtoken::errors::Error },
    #[error("Invalid project OCI token: {error}")]
    OciProject { error: jsonwebtoken::errors::Error },
}

impl TokenError {
    /// True iff this error was produced by an HMAC signature mismatch during
    /// decode. The auth path uses this to decide whether to retry validation
    /// against previous (rotated-out) signing keys; any other error kind
    /// (audience mismatch, malformed shape, expiration, etc.) must short-circuit
    /// because it is independent of which key was used.
    #[must_use]
    pub fn is_invalid_signature(&self) -> bool {
        matches!(
            self,
            Self::Decode { error }
                if matches!(error.kind(), jsonwebtoken::errors::ErrorKind::InvalidSignature)
        )
    }
}
