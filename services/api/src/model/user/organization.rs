use crate::{
    model::{organization::QueryOrganization, user::auth::INVALID_JWT},
    schema::organization_role as organization_role_table,
    util::context::SecretKey,
    ApiError,
};
use bencher_json::jwt::JsonWebToken;
use diesel::{Insertable, Queryable, SqliteConnection};

use super::{
    auth::{auth_header_error, map_auth_header_error},
    QueryUser,
};

#[derive(Insertable)]
#[diesel(table_name = organization_role_table)]
pub struct InsertOrganizationRole {
    pub user_id: i32,
    pub organization_id: i32,
    pub role: String,
}

impl InsertOrganizationRole {
    pub fn from_jwt(
        conn: &mut SqliteConnection,
        invite: &JsonWebToken,
        secret_key: &SecretKey,
        user_id: i32,
    ) -> Result<Self, ApiError> {
        // Validate the invite JWT
        let token_data = invite
            .validate_invite(&secret_key.decoding)
            .map_err(map_auth_header_error!(INVALID_JWT))?;

        // Make sure that there is an `org` field in the claims
        let org_claims = token_data
            .claims
            .org()
            .ok_or_else(auth_header_error!(INVALID_JWT))?;

        // Make sure the email in the invite is the same as the email associated with the user
        let email_user_id = QueryUser::get_id_from_email(conn, token_data.claims.email())?;
        if user_id != email_user_id {
            return Err(ApiError::InviteEmail {
                user_id,
                email: token_data.claims.email().into(),
                email_user_id,
            });
        }

        Ok(InsertOrganizationRole {
            user_id,
            organization_id: QueryOrganization::get_id(conn, org_claims.uuid)?,
            role: serde_json::to_string(&org_claims.role).map_err(ApiError::Serialize)?,
        })
    }
}

#[derive(Queryable)]
pub struct QueryOrganizationRole {
    pub id: i32,
    pub user_id: i32,
    pub organization_id: i32,
    pub role: String,
}
