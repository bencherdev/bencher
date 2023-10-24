use bencher_json::Jwt;

use crate::claims::Claims;

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("Failed to encode JSON Web Token: {error}")]
    Encode {
        claims: Claims,
        error: jsonwebtoken::errors::Error,
    },
    #[error("Failed to decode JSON Web Token: {error}")]
    Decode {
        token: Jwt,
        error: jsonwebtoken::errors::Error,
    },
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
}
