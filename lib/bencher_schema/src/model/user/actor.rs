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
    error::{issue_error, unauthorized_error},
    model::{
        project::{
            ProjectId,
            key::{ProjectKeyId, QueryProjectKey},
        },
        user::UserId,
    },
};

use super::public::{PubBearerToken, PublicUser};

const INVALID_PROJECT_KEY: &str = "Invalid project key";

pub enum ApiActor {
    Public(PublicUser),
    ProjectKey(ProjectKeyActor),
}

pub struct ProjectKeyActor {
    pub key_id: ProjectKeyId,
    pub project_id: ProjectId,
}

impl ProjectKeyActor {
    pub fn verify_project(&self, project_id: ProjectId) -> Result<(), HttpError> {
        if project_id == self.project_id {
            Ok(())
        } else {
            Err(unauthorized_error(INVALID_PROJECT_KEY))
        }
    }
}

impl From<super::auth::AuthUser> for ApiActor {
    fn from(auth_user: super::auth::AuthUser) -> Self {
        Self::Public(PublicUser::from(auth_user))
    }
}

pub struct PubProjectBearerToken(Option<String>);

#[async_trait]
impl SharedExtractor for PubProjectBearerToken {
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
    pub async fn new(rqctx: &RequestContext<ApiContext>) -> Result<Self, HttpError> {
        let pub_project_bearer_token = PubProjectBearerToken::from_request(rqctx).await?;
        Self::from_token(
            &rqctx.log,
            rqctx.context(),
            #[cfg(feature = "plus")]
            rqctx.request.headers(),
            pub_project_bearer_token,
        )
        .await
    }

    pub fn is_auth(&self) -> bool {
        match self {
            Self::Public(public_user) => public_user.is_auth(),
            Self::ProjectKey(_) => true,
        }
    }

    pub fn user_id(&self) -> Option<UserId> {
        match self {
            Self::Public(public_user) => public_user.user_id(),
            Self::ProjectKey(_) => None,
        }
    }

    pub async fn from_token(
        log: &Logger,
        context: &ApiContext,
        #[cfg(feature = "plus")] headers: &crate::HeaderMap,
        pub_project_bearer_token: PubProjectBearerToken,
    ) -> Result<Self, HttpError> {
        let raw_bearer = pub_project_bearer_token.0.as_deref();

        if let Some(raw) = raw_bearer.filter(|r| r.starts_with(PROJECT_KEY_PREFIX)) {
            return Self::authenticate_project_key(log, context, raw).await;
        }

        let pub_bearer_token = match raw_bearer {
            Some(raw) => {
                let jwt = raw
                    .parse::<bencher_json::Jwt>()
                    .map_err(|_err| unauthorized_error("Invalid authorization token"))?;
                PubBearerToken::from(Some(jwt.into()))
            },
            None => PubBearerToken::from(None),
        };
        let public_user = PublicUser::from_token(
            log,
            context,
            #[cfg(feature = "plus")]
            headers,
            pub_bearer_token,
        )
        .await?;
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
            unauthorized_error(INVALID_PROJECT_KEY)
        })?;
        let key_hash = ProjectKeyHash::from(&key);

        let now = context.clock.now();
        let query_key = QueryProjectKey::from_hash(auth_conn!(context), &key_hash, now)
            .optional()
            .inspect_err(|err| {
                issue_error(
                    "Failed to lookup project key",
                    &format!("Failed to lookup project key by hash: {key_hash}"),
                    err,
                );
            })
            .map_err(|_err| unauthorized_error(INVALID_PROJECT_KEY))?
            .ok_or_else(|| {
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::ProjectKeyAuthFailed(
                    bencher_otel::ProjectKeyAuthFailureReason::NotFound,
                ));
                unauthorized_error(INVALID_PROJECT_KEY)
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
