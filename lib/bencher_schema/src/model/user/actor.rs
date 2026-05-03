use async_trait::async_trait;
use bencher_json::{PROJECT_KEY_PREFIX, ProjectKey, ProjectKeyHash};
use diesel::OptionalExtension as _;
use dropshot::{
    ApiEndpointBodyContentType, ExtensionMode, ExtractorMetadata, HttpError, RequestContext,
    ServerContext, SharedExtractor,
};
use slog::Logger;

use crate::{
    ApiContext, auth_conn,
    error::unauthorized_error,
    model::project::{
        ProjectId,
        key::{ProjectKeyId, QueryProjectKey},
    },
};

use super::{auth::AuthUser, public::PublicUser};

pub enum ApiActor {
    Public(PublicUser),
    ProjectKey(ProjectKeyActor),
}

pub struct ProjectKeyActor {
    pub key_id: ProjectKeyId,
    pub project_id: ProjectId,
}

pub struct PubApiActor(Option<String>);

#[async_trait]
impl SharedExtractor for PubApiActor {
    async fn from_request<Context: ServerContext>(
        rqctx: &RequestContext<Context>,
    ) -> Result<Self, HttpError> {
        let raw = rqctx
            .request
            .headers()
            .get(bencher_json::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(bencher_json::strip_bearer_token)
            .map(String::from);
        Ok(Self(raw))
    }

    fn metadata(_body_content_type: ApiEndpointBodyContentType) -> ExtractorMetadata {
        ExtractorMetadata {
            extension_mode: ExtensionMode::None,
            parameters: Vec::new(),
        }
    }
}

impl ApiActor {
    pub async fn from_token(
        log: &Logger,
        context: &ApiContext,
        #[cfg(feature = "plus")] headers: &crate::HeaderMap,
        pub_api_actor: PubApiActor,
    ) -> Result<Self, HttpError> {
        let raw_bearer = pub_api_actor.0.as_deref();

        if let Some(raw) = raw_bearer.filter(|r| r.starts_with(PROJECT_KEY_PREFIX)) {
            return Self::authenticate_project_key(log, context, raw).await;
        }

        let public_user = if let Some(raw) = raw_bearer {
            let jwt = raw
                .parse::<bencher_json::Jwt>()
                .map_err(|_err| unauthorized_error("Invalid authorization token"))?;
            let user = AuthUser::from_token(context, jwt.into()).await?;
            slog::info!(log, "Authenticated user"; "user_uuid" => %user.user.uuid);
            PublicUser::Auth(Box::new(user))
        } else {
            #[cfg(feature = "plus")]
            let remote_ip = {
                let remote_ip = crate::RateLimiting::remote_ip(log, headers);
                remote_ip
                    .map(|ip| context.rate_limiting.public_request(ip))
                    .transpose()?;
                remote_ip
            };
            #[cfg(not(feature = "plus"))]
            let remote_ip = None;

            PublicUser::Public(remote_ip)
        };

        Ok(Self::Public(public_user))
    }

    async fn authenticate_project_key(
        log: &Logger,
        context: &ApiContext,
        raw: &str,
    ) -> Result<Self, HttpError> {
        let key: ProjectKey = raw.parse().map_err(|_err| {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ProjectKeyAuthFailed(
                bencher_otel::ProjectKeyAuthFailureReason::Invalid,
            ));
            unauthorized_error("Invalid project key")
        })?;
        let key_hash = ProjectKeyHash::from(&key);

        let now = context.clock.now();
        let query_key = QueryProjectKey::from_hash(auth_conn!(context), &key_hash, now)
            .optional()
            .map_err(|err| {
                slog::error!(log, "DB error during project key lookup"; "error" => %err);
                unauthorized_error("Invalid project key")
            })?
            .ok_or_else(|| {
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ProjectKeyAuthFailed(
                    bencher_otel::ProjectKeyAuthFailureReason::NotFound,
                ));
                unauthorized_error("Invalid project key")
            })?;

        slog::info!(
            log,
            "Authenticated project key";
            "project_key_uuid" => %query_key.uuid,
            "project_id" => ?query_key.project_id
        );

        let key_id = query_key.id;
        let project_id = query_key.project_id;

        Ok(Self::ProjectKey(ProjectKeyActor { key_id, project_id }))
    }
}
