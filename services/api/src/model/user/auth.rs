use bencher_rbac::User;
use std::str::FromStr;

use bencher_json::{jwt::JsonWebToken, JsonSignup, JsonUser, ResourceId};
use diesel::{
    expression_methods::BoolExpressionMethods, Insertable, QueryDsl, Queryable, RunQueryDsl,
    SqliteConnection,
};
use dropshot::{HttpError, RequestContext};
use email_address_parser::EmailAddress;
use uuid::Uuid;

use crate::{
    diesel::ExpressionMethods,
    schema::{self, user as user_table},
    util::{http_error, map_http_error, slug::unwrap_slug, Context},
    ApiError,
};

pub struct AuthUser {
    pub id: i32,
    pub rbac: User,
}

impl AuthUser {
    pub async fn new(rqctx: &RequestContext<Context>) -> Result<Self, ApiError> {
        let request = rqctx.request.lock().await;

        let headers = request
            .headers()
            .get("Authorization")
            .ok_or_else(|| http_error!("Missing \"Authorization\" header."))?
            .to_str()
            .map_err(map_http_error!("Invalid \"Authorization\" header."))?;
        let (_, token) = headers
            .split_once("Bearer ")
            .ok_or_else(|| http_error!("Missing \"Authorization\" Bearer."))?;
        let jwt: JsonWebToken = token.to_string().into();

        let context = &mut *rqctx.context().lock().await;
        let token_data = jwt
            .validate_user(&context.secret_key)
            .map_err(map_http_error!("Invalid JWT (JSON Web Token)."))?;

        let conn = &mut context.db_conn;
        schema::user::table
            .filter(schema::user::email.eq(token_data.claims.email()))
            .select(schema::user::id)
            .first::<i32>(conn)
            .map_err(map_http_error!("Invalid JWT (JSON Web Token)."));

        todo!()
    }
}
