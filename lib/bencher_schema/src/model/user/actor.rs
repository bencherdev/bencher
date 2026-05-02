use bencher_json::{PROJECT_KEY_PREFIX, ProjectKey, ProjectKeyHash};
use diesel::{ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;
use slog::Logger;

use crate::{
    ApiContext, auth_conn,
    error::unauthorized_error,
    model::project::{
        ProjectId,
        key::{ProjectKeyId, QueryProjectKey},
    },
    schema, write_conn,
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

impl ApiActor {
    pub async fn new(rqctx: &dropshot::RequestContext<ApiContext>) -> Result<Self, HttpError> {
        let headers = rqctx.request.headers();
        let raw_bearer = headers
            .get(bencher_json::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(bencher_json::strip_bearer_token);

        Self::from_raw_bearer(
            &rqctx.log,
            rqctx.context(),
            #[cfg(feature = "plus")]
            headers,
            raw_bearer,
        )
        .await
    }

    async fn from_raw_bearer(
        log: &Logger,
        context: &ApiContext,
        #[cfg(feature = "plus")] headers: &crate::HeaderMap,
        raw_bearer: Option<&str>,
    ) -> Result<Self, HttpError> {
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
        let key: ProjectKey = raw
            .parse()
            .map_err(|_err| unauthorized_error("Invalid project key"))?;
        let key_hash = ProjectKeyHash::from(&key);

        let query_key: QueryProjectKey = schema::project_key::table
            .filter(schema::project_key::key_hash.eq(key_hash.as_ref()))
            .filter(schema::project_key::revoked.is_null())
            .first::<QueryProjectKey>(auth_conn!(context))
            .optional()
            .map_err(|_err| unauthorized_error("Invalid project key"))?
            .ok_or_else(|| unauthorized_error("Invalid project key"))?;

        let now = context.clock.now();
        if query_key.expiration.timestamp() < now.timestamp() {
            return Err(unauthorized_error("Invalid project key"));
        }

        slog::info!(
            log,
            "Authenticated project key";
            "project_key_uuid" => %query_key.uuid,
            "project_id" => ?query_key.project_id
        );

        let key_id = query_key.id;
        let project_id = query_key.project_id;

        drop(QueryProjectKey::touch_last_used(
            write_conn!(context),
            key_id,
            now,
        ));

        Ok(Self::ProjectKey(ProjectKeyActor { key_id, project_id }))
    }
}
