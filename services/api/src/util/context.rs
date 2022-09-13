use oso::{Oso, ToPolar};

use crate::diesel::ExpressionMethods;
use crate::{
    model::{organization::QueryOrganization, user::auth::AuthUser},
    ApiError,
};

pub type Context = tokio::sync::Mutex<ApiContext>;

pub struct ApiContext {
    pub secret_key: String,
    pub rbac: Rbac,
    pub db_conn: diesel::SqliteConnection,
}

pub struct Rbac(pub Oso);

impl From<Oso> for Rbac {
    fn from(oso: Oso) -> Self {
        Self(oso)
    }
}

impl Rbac {
    pub fn is_allowed<Actor, Action, Resource>(
        &self,
        actor: Actor,
        action: Action,
        resource: Resource,
    ) -> Result<bool, ApiError>
    where
        Actor: ToPolar,
        Action: ToPolar,
        Resource: ToPolar,
    {
        self.0.is_allowed(actor, action, resource).map_err(|e| {
            let err = ApiError::IsAllowed(e);
            tracing::info!("{err}");
            err
        })
    }

    fn is_allowed_organization(
        &self,
        auth_user: &AuthUser,
        action: bencher_rbac::organization::Permission,
        organization: &QueryOrganization,
    ) -> Box<
        dyn diesel::BoxableExpression<
            crate::schema::organization::table,
            diesel::sqlite::Sqlite,
            SqlType = diesel::sql_types::Bool,
        >,
    > {
        let is = self
            .is_allowed(auth_user, action, organization)
            .unwrap_or_default();

        Box::new(crate::schema::organization::id.eq(0))
    }
}
