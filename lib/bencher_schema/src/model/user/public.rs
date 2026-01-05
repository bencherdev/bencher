use std::net::IpAddr;

use async_trait::async_trait;
use dropshot::{
    ApiEndpointBodyContentType, ExtensionMode, ExtractorMetadata, HttpError, RequestContext,
    ServerContext, SharedExtractor,
};
use slog::Logger;

#[cfg(feature = "plus")]
use crate::RateLimiting;
use crate::{
    ApiContext,
    model::user::auth::{AuthUser, BearerToken},
};

#[derive(Debug, Clone)]
pub enum PublicUser {
    Public(Option<IpAddr>),
    Auth(AuthUser),
}

impl PublicUser {
    // This is required due to a limitation in `dropshot` where only four extractors are allowed.
    pub async fn new(rqctx: &RequestContext<ApiContext>) -> Result<Self, HttpError> {
        let pub_bearer_token = PubBearerToken::from_request(rqctx).await?;
        Self::from_token(
            &rqctx.log,
            rqctx.context(),
            &rqctx.request_id,
            #[cfg(feature = "plus")]
            rqctx.request.headers(),
            pub_bearer_token,
        )
        .await
    }

    pub async fn from_token(
        log: &Logger,
        context: &ApiContext,
        request_id: &str,
        #[cfg(feature = "plus")] headers: &crate::HeaderMap,
        bearer_token: PubBearerToken,
    ) -> Result<Self, HttpError> {
        Ok(if let Some(bearer_token) = bearer_token.0 {
            let user = AuthUser::from_token(context, bearer_token).await?;
            slog::info!(
                log,
                "Authenticated user"; "request_id" => request_id, "user_uuid" => %user.user.uuid
            );
            Self::Auth(user)
        } else {
            #[cfg(feature = "plus")]
            let remote_ip = {
                let remote_ip = RateLimiting::remote_ip(log, request_id, headers);
                remote_ip
                    .map(|ip| context.rate_limiting.public_request(ip))
                    .transpose()?;
                remote_ip
            };
            #[cfg(not(feature = "plus"))]
            let remote_ip = None;

            Self::Public(remote_ip)
        })
    }
}

pub struct PubBearerToken(Option<BearerToken>);

#[async_trait]
impl SharedExtractor for PubBearerToken {
    async fn from_request<Context: ServerContext>(
        rqctx: &RequestContext<Context>,
    ) -> Result<Self, HttpError> {
        Ok(Self(BearerToken::from_request(rqctx).await.ok()))
    }

    fn metadata(_body_content_type: ApiEndpointBodyContentType) -> ExtractorMetadata {
        ExtractorMetadata {
            extension_mode: ExtensionMode::None,
            parameters: Vec::new(),
        }
    }
}
