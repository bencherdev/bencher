use bencher_json::project::JsonVisibility;

use crate::ApiError;

const PUBLIC_INT: i32 = 0;
#[cfg(feature = "plus")]
const PRIVATE_INT: i32 = 1;

#[derive(Debug, Clone, Copy, diesel::FromSqlRow, diesel::AsExpression)]
#[diesel(sql_type = diesel::sql_types::Integer)]
#[repr(i32)]
pub enum Visibility {
    Public = PUBLIC_INT,
    #[cfg(feature = "plus")]
    Private = PRIVATE_INT,
}

impl TryFrom<i32> for Visibility {
    type Error = ApiError;

    fn try_from(visibility: i32) -> Result<Self, Self::Error> {
        match visibility {
            PUBLIC_INT => Ok(Self::Public),
            #[cfg(feature = "plus")]
            PRIVATE_INT => Ok(Self::Private),
            _ => Err(ApiError::VisibilityInt(visibility)),
        }
    }
}

impl From<JsonVisibility> for Visibility {
    fn from(visibility: JsonVisibility) -> Self {
        match visibility {
            JsonVisibility::Public => Self::Public,
            #[cfg(feature = "plus")]
            JsonVisibility::Private => Self::Private,
        }
    }
}

impl From<Visibility> for JsonVisibility {
    fn from(visibility: Visibility) -> Self {
        match visibility {
            Visibility::Public => Self::Public,
            #[cfg(feature = "plus")]
            Visibility::Private => Self::Private,
        }
    }
}

impl Visibility {
    pub fn is_public(self) -> bool {
        JsonVisibility::from(self).is_public()
    }
}

#[cfg(not(feature = "plus"))]
pub mod project_visibility {
    use bencher_json::project::JsonVisibility;

    use crate::ApiError;

    pub fn project_visibility(visibility: Option<JsonVisibility>) -> Result<(), ApiError> {
        visibility
            .unwrap_or_default()
            .is_public()
            .then_some(())
            .ok_or(ApiError::CreatePrivateProject)
    }
}

#[cfg(feature = "plus")]
pub mod project_visibility {
    use bencher_billing::Biller;
    use bencher_json::{project::JsonVisibility, ResourceId};
    use bencher_license::Licensor;

    use crate::{context::DbConnection, model::organization::QueryOrganization, ApiError};

    pub async fn project_visibility(
        conn: &mut DbConnection,
        biller: Option<&Biller>,
        licensor: &Licensor,
        organization: &ResourceId,
        visibility: Option<JsonVisibility>,
    ) -> Result<(), ApiError> {
        if visibility.unwrap_or_default().is_public() {
            Ok(())
        } else {
            check_plan(conn, biller, licensor, organization).await
        }
    }

    async fn check_plan(
        conn: &mut DbConnection,
        biller: Option<&Biller>,
        licensor: &Licensor,
        organization: &ResourceId,
    ) -> Result<(), ApiError> {
        if let Some(subscription) = QueryOrganization::get_subscription(conn, organization)? {
            if let Some(biller) = biller {
                let plan_status = biller.get_plan_status(&subscription).await?;
                if plan_status.is_active() {
                    Ok(())
                } else {
                    Err(ApiError::InactivePlanOrganization(organization.clone()))
                }
            } else {
                Err(ApiError::NoBillerOrganization(organization.clone()))
            }
        } else if let Some((uuid, license)) = QueryOrganization::get_license(conn, organization)? {
            let _token_data = licensor.validate_organization(&license, uuid.into())?;
            // TODO check license entitlements for usage so far
            Ok(())
        } else {
            Err(ApiError::NoPlanOrganization(organization.clone()))
        }
    }
}

impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for Visibility
where
    DB: diesel::backend::Backend,
    i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        match self {
            Self::Public => PUBLIC_INT.to_sql(out),
            #[cfg(feature = "plus")]
            Self::Private => PRIVATE_INT.to_sql(out),
        }
    }
}

impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for Visibility
where
    DB: diesel::backend::Backend,
    i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        Ok(Self::try_from(i32::from_sql(bytes)?)?)
    }
}
