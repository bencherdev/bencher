use dropshot::HttpError;

use crate::{context::ApiContext, error::forbidden_error};

use super::auth::{AuthUser, BearerToken};

pub struct AdminUser(AuthUser);

impl AdminUser {
    pub async fn from_token(
        context: &ApiContext,
        bearer_token: BearerToken,
    ) -> Result<Self, HttpError> {
        let auth_user = AuthUser::from_token(context, bearer_token).await?;
        if !auth_user.is_admin(&context.rbac) {
            return Err(forbidden_error(format!(
                "User is not an admin ({auth_user:?})."
            )));
        }
        Ok(Self(auth_user))
    }

    pub fn user(&self) -> &AuthUser {
        &self.0
    }
}
