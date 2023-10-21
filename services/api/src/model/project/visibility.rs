#[cfg(not(feature = "plus"))]
pub mod project_visibility {
    use bencher_json::project::Visibility;
    use dropshot::HttpError;

    pub fn project_visibility(visibility: Option<Visibility>) -> Result<(), HttpError> {
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
    use bencher_json::{project::Visibility, ResourceId};
    use bencher_license::Licensor;
    use dropshot::HttpError;
    use http::StatusCode;

    use crate::{
        context::DbConnection,
        error::{issue_error, not_found_error, payment_required_error},
        model::organization::{plan::QueryPlan, QueryOrganization},
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
        visibility: Option<Visibility>,
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
        let query_plan = QueryPlan::belonging_to(&query_organization).first::<QueryPlan>(conn)?;
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
