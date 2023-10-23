#![cfg(feature = "plus")]

use bencher_billing::{Biller, Customer, PaymentMethod};
use bencher_json::{
    organization::plan::JsonLicense, project::Visibility, DateTime, JsonPlan, Jwt, LicensedPlanId,
    MeteredPlanId, OrganizationUuid, PlanLevel,
};
use bencher_license::Licensor;
use diesel::{BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;
use http::StatusCode;

use crate::{
    context::DbConnection,
    error::{
        bad_request_error, issue_error, not_found_error, payment_required_error,
        resource_conflict_err, resource_not_found_err,
    },
    model::{
        organization::{OrganizationId, QueryOrganization, UpdateOrganization},
        project::{metric::QueryMetric, QueryProject},
    },
    schema::{self, plan as plan_table},
};

crate::util::typed_id::typed_id!(PlanId);

// Metrics are bundled by the thousand for licensed plans
pub const ENTITLEMENTS_QUANTITY: u64 = 1_000;

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

impl QueryPlan {
    pub async fn metered_plan(&self, biller: &Biller) -> Result<Option<JsonPlan>, HttpError> {
        let Some(metered_plan_id) = self.metered_plan.clone() else {
            return Ok(None);
        };

        biller
            .get_plan(metered_plan_id)
            .await
            .map(Some)
            .map_err(resource_not_found_err!(Plan, self))
    }

    pub async fn licensed_plan(
        &self,
        biller: &Biller,
        licensor: &Licensor,
        organization_uuid: OrganizationUuid,
    ) -> Result<Option<JsonPlan>, HttpError> {
        let Some(licensed_plan_id) = self.licensed_plan.clone() else {
            return Ok(None);
        };

        let mut json_plan = biller
            .get_plan(licensed_plan_id)
            .await
            .map_err(resource_not_found_err!(Plan, self))?;

        let Some(license) = self.license.as_ref() else {
            return Err(issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to find license for licensed plan",
                &format!(
                    "Failed to find license for plan ({self:?}) even though licensed plan exists ({json_plan:?}).",
                ),
                "Failed to find license for licensed plan",
            ));
        };

        let token_data = licensor
            .validate_organization(license, organization_uuid)
            .map_err(payment_required_error)?;

        let json_license = JsonLicense {
            key: license.clone(),
            organization: organization_uuid,
            entitlements: token_data.claims.entitlements(),
            issued_at: token_data.claims.issued_at(),
            expiration: token_data.claims.expiration(),
        };
        json_plan.license = Some(json_license);

        Ok(Some(json_plan))
    }
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

impl InsertPlan {
    pub async fn metered_plan(
        conn: &mut DbConnection,
        biller: &Biller,
        query_organization: &QueryOrganization,
        customer: &Customer,
        payment_method: &PaymentMethod,
        plan_level: PlanLevel,
        price_name: String,
    ) -> Result<Self, HttpError> {
        // Create a metered subscription for the organization
        let subscription = biller
            .create_metered_subscription(
                query_organization.uuid,
                customer,
                payment_method,
                plan_level,
                price_name.clone(),
            )
            .await
            .map_err(resource_conflict_err!(
                Plan,
                (&query_organization, customer, plan_level, price_name)
            ))?;

        let metered_plan_id: MeteredPlanId = subscription
            .id
            .as_ref()
            .parse()
            .map_err(resource_not_found_err!(Plan, subscription))?;
        let timestamp = DateTime::now();

        let insert_plan = InsertPlan {
            organization_id: query_organization.id,
            metered_plan: Some(metered_plan_id),
            licensed_plan: None,
            license: None,
            created: timestamp,
            modified: timestamp,
        };

        diesel::insert_into(schema::plan::table)
            .values(&insert_plan)
            .execute(conn)
            .map_err(resource_conflict_err!(Plan, insert_plan))?;

        Ok(insert_plan)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn licensed_plan(
        conn: &mut DbConnection,
        biller: &Biller,
        licensor: &Licensor,
        query_organization: &QueryOrganization,
        customer: &Customer,
        payment_method: &PaymentMethod,
        plan_level: PlanLevel,
        price_name: String,
        license_entitlements: u64,
        license_organization: Option<OrganizationUuid>,
    ) -> Result<Self, HttpError> {
        // TODO make an entitlements type
        // TODO need to move stripe over to single unit costs for licenses
        if license_entitlements == 0 {
            return Err(bad_request_error(format!(
                "Entitlements ({license_entitlements}) must be a multiple of 1000",
            )));
        }

        // Create a licensed subscription for the organization
        let subscription = biller
            .create_licensed_subscription(
                query_organization.uuid,
                customer,
                payment_method,
                plan_level,
                price_name.clone(),
                license_entitlements,
            )
            .await
            .map_err(resource_conflict_err!(
                Plan,
                (
                    &query_organization,
                    customer,
                    plan_level,
                    price_name,
                    license_entitlements
                )
            ))?;

        let licensed_plan_id: LicensedPlanId = subscription
            .id
            .as_ref()
            .parse()
            .map_err(resource_not_found_err!(Plan, subscription))?;

        // If license organization is not given, then use the current organization (Bencher Cloud license)
        let organization_uuid = license_organization.unwrap_or(query_organization.uuid);
        let license = licensor
            .new_annual_license(organization_uuid, license_entitlements)
            .map_err(|e| issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create license",
                &format!("Failed to create license for organization ({query_organization:?}) with entitlements ({license_entitlements})."),
                e,
            ))?;
        let timestamp = DateTime::now();

        let insert_plan = InsertPlan {
            organization_id: query_organization.id,
            metered_plan: None,
            licensed_plan: Some(licensed_plan_id),
            license: Some(license.clone()),
            created: timestamp,
            modified: timestamp,
        };

        diesel::insert_into(schema::plan::table)
            .values(&insert_plan)
            .execute(conn)
            .map_err(resource_conflict_err!(Plan, insert_plan))?;

        // If the license is for this organization is not given, then update the current organization (Bencher Cloud license)
        if license_organization.is_none() {
            let organization_query = schema::organization::table
                .filter(schema::organization::id.eq(query_organization.id));
            let update_organization = UpdateOrganization {
                name: None,
                slug: None,
                license: Some(Some(license)),
                modified: timestamp,
            };
            diesel::update(organization_query)
                .set(&update_organization)
                .execute(conn)
                .map_err(resource_conflict_err!(Organization, update_organization))?;
        }

        Ok(insert_plan)
    }
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
    // It is important that we check the organization license and NOT the plan license
    // The organization license is the one that is actually in use, either on Bencher Cloud or Self-Hosted
    // The plan license is simply there to keep track of the license on Bencher Cloud only
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
