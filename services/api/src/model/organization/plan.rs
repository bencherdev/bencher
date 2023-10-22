#![cfg(feature = "plus")]

use bencher_billing::Biller;
use bencher_json::{project::Visibility, DateTime, Jwt, LicensedPlanId, MeteredPlanId};
use bencher_license::Licensor;
use diesel::{BelongingToDsl, RunQueryDsl};
use dropshot::HttpError;
use http::StatusCode;

use crate::{
    context::DbConnection,
    error::{issue_error, not_found_error, payment_required_error},
    model::{
        organization::{OrganizationId, QueryOrganization},
        project::{metric::QueryMetric, QueryProject},
    },
    schema::plan as plan_table,
};

crate::util::typed_id::typed_id!(PlanId);

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = plan_table)]
#[diesel(belongs_to(QueryOrganization, foreign_key = organization_id))]
pub struct QueryPlan {
    pub id: PlanId,
    pub organization_id: OrganizationId,
    pub metered_plan: Option<MeteredPlanId>,
    pub licensed_plan: Option<LicensedPlanId>,
    pub license: Option<Jwt>,
    pub created: DateTime,
    pub modified: DateTime,
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = plan_table)]
pub struct InsertPlan {
    pub organization_id: OrganizationId,
    pub metered_plan: Option<MeteredPlanId>,
    pub licensed_plan: Option<LicensedPlanId>,
    pub license: Option<Jwt>,
    pub created: DateTime,
    pub modified: DateTime,
}

pub enum PlanKind {
    Metered(MeteredPlanId),
    Licensed(LicenseUsage),
    None,
}

pub struct LicenseUsage {
    pub entitlements: u64,
    pub usage: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum PlanKindError {
    #[error("Organization ({organization:?}) has an inactive metered plan ({metered_plan_id})")]
    InactiveMeteredPlan {
        organization: QueryOrganization,
        metered_plan_id: MeteredPlanId,
    },
    #[error("No plan (subscription or license) found for organization ({organization:?}) with private project ({visibility:?})")]
    NoPlan {
        organization: QueryOrganization,
        visibility: Visibility,
    },
    #[error("No Biller has been configured for the server")]
    NoBiller,
    #[error("License usage exceeded for project ({project:?}). {prior_usage} + {usage} > {entitlements}")]
    Overage {
        project: QueryProject,
        entitlements: u64,
        prior_usage: u64,
        usage: u64,
    },
}

impl PlanKind {
    pub async fn new(
        conn: &mut DbConnection,
        biller: Option<&Biller>,
        licensor: &Licensor,
        query_organization: &QueryOrganization,
        visibility: Visibility,
    ) -> Result<Self, HttpError> {
        if let Some(metered_plan_id) = get_metered_plan(conn, biller, query_organization).await? {
            Ok(Self::Metered(metered_plan_id))
        } else if let Some(license_usage) = get_license_usage(conn, licensor, query_organization)? {
            Ok(Self::Licensed(license_usage))
        } else if visibility.is_public() {
            Ok(Self::None)
        } else {
            Err(payment_required_error(PlanKindError::NoPlan {
                organization: query_organization.clone(),
                visibility,
            }))
        }
    }

    pub async fn new_for_project(
        conn: &mut DbConnection,
        biller: Option<&Biller>,
        licensor: &Licensor,
        project: &QueryProject,
    ) -> Result<Self, HttpError> {
        let query_organization = QueryOrganization::get(conn, project.organization_id)?;
        Self::new(
            conn,
            biller,
            licensor,
            &query_organization,
            project.visibility,
        )
        .await
    }

    pub async fn check_usage(
        self,
        biller: Option<&Biller>,
        project: &QueryProject,
        usage: u64,
    ) -> Result<(), HttpError> {
        match self {
            Self::Metered(metered_plan_id) => {
                let Some(biller) = biller else {
                    return Err(issue_error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "No Biller when checking usage",
                        "Failed to find Biller in Bencher Cloud when checking usage.",
                        PlanKindError::NoBiller,
                    ));
                };
                biller
                    .record_usage(metered_plan_id, usage)
                    .await
                    .map_err(|e| {
                        issue_error(
                            StatusCode::BAD_REQUEST,
                            "Failed to record usage",
                            &format!("Failed to record usage ({usage}) in project ({project_id}) on Bencher.", project_id= project.id),
                            e,
                        )
                    })?;
            },
            Self::Licensed(LicenseUsage {
                entitlements,
                usage: prior_usage,
            }) => {
                if prior_usage + usage > entitlements {
                    return Err(payment_required_error(PlanKindError::Overage {
                        project: project.clone(),
                        entitlements,
                        prior_usage,
                        usage,
                    }));
                }
            },
            Self::None => {},
        }

        Ok(())
    }
}

async fn get_metered_plan(
    conn: &mut DbConnection,
    biller: Option<&Biller>,
    query_organization: &QueryOrganization,
) -> Result<Option<MeteredPlanId>, HttpError> {
    let Some(biller) = biller else {
        return Ok(None);
    };

    let Ok(query_plan) = QueryPlan::belonging_to(&query_organization).first::<QueryPlan>(conn)
    else {
        return Ok(None);
    };

    let Some(metered_plan_id) = query_plan.metered_plan.clone() else {
        return Ok(None);
    };

    let plan_status = biller
        .get_plan_status(metered_plan_id.clone())
        .await
        .map_err(not_found_error)?;

    if plan_status.is_active() {
        Ok(Some(metered_plan_id))
    } else {
        Err(payment_required_error(PlanKindError::InactiveMeteredPlan {
            organization: query_organization.clone(),
            metered_plan_id,
        }))
    }
}

fn get_license_usage(
    conn: &mut DbConnection,
    licensor: &Licensor,
    query_organization: &QueryOrganization,
) -> Result<Option<LicenseUsage>, HttpError> {
    let Some(license) = &query_organization.license else {
        return Ok(None);
    };

    let token_data = licensor
        .validate_organization(license, query_organization.uuid)
        .map_err(payment_required_error)?;

    let start_time = token_data.claims.issued_at();
    let end_time = token_data.claims.expiration();

    let usage = QueryMetric::usage(conn, query_organization.id, start_time, end_time)?;
    let entitlements = licensor
        .validate_usage(&token_data.claims, usage)
        .map_err(payment_required_error)?;

    Ok(Some(LicenseUsage {
        entitlements,
        usage,
    }))
}
