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
    use dropshot::HttpError;

    pub fn project_visibility(visibility: Option<JsonVisibility>) -> Result<(), HttpError> {
        visibility
            .unwrap_or_default()
            .is_public()
            .then_some(())
            .ok_or(crate::error::payment_required_error(format!(
                "Private projects are only available with the an active Bencher Plus plan. Please upgrade your plan at: https://bencher.dev/pricing"
            )))
    }
}

#[cfg(feature = "plus")]
pub mod project_visibility {
    use bencher_billing::{Biller, SubscriptionId};
    use bencher_json::{project::JsonVisibility, ResourceId};
    use bencher_license::Licensor;
    use dropshot::HttpError;
    use http::StatusCode;

    use crate::{
        context::DbConnection,
        error::{issue_error, not_found_error, payment_required_error},
        model::organization::QueryOrganization,
    };

    #[derive(Debug, thiserror::Error)]
    pub enum ProjectVisibilityError {
        #[error("Organization ({organization:?}) has an inactive plan ({subscription_id})")]
        InactivePlan {
            organization: ResourceId,
            subscription_id: SubscriptionId,
        },
        #[error("No Biller has been configured for the server.")]
        NoBiller,
        #[error("No plan (subscription or license) found for organization ({0})")]
        NoPlan(ResourceId),
    }

    pub async fn project_visibility(
        conn: &mut DbConnection,
        biller: Option<&Biller>,
        licensor: &Licensor,
        organization: &ResourceId,
        visibility: Option<JsonVisibility>,
    ) -> Result<(), HttpError> {
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
    ) -> Result<(), HttpError> {
        let query_organization = QueryOrganization::from_resource_id(conn, organization)?;
        if let Some(subscription_id) = query_organization.get_subscription()? {
            if let Some(biller) = biller {
                let plan_status = biller
                    .get_plan_status(&subscription_id)
                    .await
                    .map_err(not_found_error)?;
                if plan_status.is_active() {
                    Ok(())
                } else {
                    Err(payment_required_error(
                        ProjectVisibilityError::InactivePlan {
                            organization: organization.clone(),
                            subscription_id,
                        },
                    ))
                }
            } else {
                Err(issue_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "No Biller when checking plan kind",
                    "Failed to find Biller in Bencher Cloud when checking plan kind for organization.",
                    ProjectVisibilityError::NoBiller,
                ))
            }
        } else if let Some(license) = query_organization.get_license()? {
            query_organization
                .check_license_usage(conn, licensor, &license)
                .map(|_| ())
        } else {
            Err(payment_required_error(ProjectVisibilityError::NoPlan(
                organization.clone(),
            )))
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
