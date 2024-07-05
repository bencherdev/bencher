#![cfg(feature = "plus")]

use bencher_billing::Biller;
use bencher_json::{
    project::Visibility, DateTime, Entitlements, JsonPlan, Jwt, LicensedPlanId, MeteredPlanId,
    OrganizationUuid, PlanLevel,
};
use bencher_license::Licensor;
use diesel::{BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;
use http::StatusCode;

use crate::{
    context::DbConnection,
    error::{
        issue_error, not_found_error, payment_required_error, resource_conflict_err,
        resource_not_found_err,
    },
    model::{
        organization::{OrganizationId, QueryOrganization, UpdateOrganization},
        project::{metric::QueryMetric, QueryProject},
    },
    schema::{self, plan as plan_table},
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

impl QueryPlan {
    pub async fn to_metered_plan(&self, biller: &Biller) -> Result<Option<JsonPlan>, HttpError> {
        let Some(metered_plan_id) = &self.metered_plan else {
            return Ok(None);
        };

        biller
            .get_metered_plan(metered_plan_id)
            .await
            .map(Some)
            .map_err(resource_not_found_err!(Plan, self))
    }

    pub async fn to_licensed_plan(
        &self,
        biller: &Biller,
        licensor: &Licensor,
        organization_uuid: OrganizationUuid,
    ) -> Result<Option<JsonPlan>, HttpError> {
        let Some(licensed_plan_id) = &self.licensed_plan else {
            return Ok(None);
        };

        let mut json_plan = biller
            .get_licensed_plan(licensed_plan_id)
            .await
            .map_err(resource_not_found_err!(Plan, self))?;

        let Some(license) = self.license.clone() else {
            return Err(issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to find license for licensed plan",
                &format!(
                    "Failed to find license for plan ({self:?}) even though licensed plan exists ({json_plan:?}).",
                ),
                "Failed to find license for licensed plan",
            ));
        };

        let json_license = licensor
            .into_json(license, Some(organization_uuid))
            .map_err(payment_required_error)?;
        json_plan.license = Some(json_license);

        Ok(Some(json_plan))
    }

    pub async fn get_active_metered_plan(
        conn: &mut DbConnection,
        biller: Option<&Biller>,
        query_organization: &QueryOrganization,
    ) -> Result<Option<MeteredPlanId>, HttpError> {
        let Some(biller) = biller else {
            return Ok(None);
        };

        let Ok(query_plan) = Self::belonging_to(&query_organization).first::<QueryPlan>(conn)
        else {
            return Ok(None);
        };

        let Some(metered_plan_id) = query_plan.metered_plan.clone() else {
            return Ok(None);
        };

        let plan_status = biller
            .get_metered_plan_status(&metered_plan_id)
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
    pub fn metered_plan(
        conn: &mut DbConnection,
        metered_plan_id: MeteredPlanId,
        query_organization: &QueryOrganization,
    ) -> Result<Self, HttpError> {
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

    pub fn licensed_plan(
        conn: &mut DbConnection,
        licensor: &Licensor,
        licensed_plan_id: LicensedPlanId,
        query_organization: &QueryOrganization,
        plan_level: PlanLevel,
        entitlements: Entitlements,
        self_hosted: Option<OrganizationUuid>,
    ) -> Result<Self, HttpError> {
        // If license organization is not given, then use the current organization (Bencher Cloud license)
        let organization_uuid = self_hosted.unwrap_or(query_organization.uuid);
        let license = licensor
            .new_annual_license(organization_uuid, plan_level, entitlements)
            .map_err(|e| issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create license",
                &format!("Failed to create license for organization ({query_organization:?}) with entitlements ({entitlements})."),
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
        if self_hosted.is_none() {
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

#[allow(variant_size_differences)]
pub enum PlanKind {
    Metered(MeteredPlanId),
    Licensed(LicenseUsage),
    None,
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
        entitlements: Entitlements,
        prior_usage: u32,
        usage: u32,
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
        if let Some(metered_plan_id) =
            QueryPlan::get_active_metered_plan(conn, biller, query_organization).await?
        {
            Ok(Self::Metered(metered_plan_id))
        } else if let Some(license_usage) = LicenseUsage::get(conn, licensor, query_organization)? {
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

    pub async fn check_for_organization(
        conn: &mut DbConnection,
        biller: Option<&Biller>,
        licensor: &Licensor,
        query_organization: &QueryOrganization,
        visibility: Visibility,
    ) -> Result<(), HttpError> {
        // Check if the project is public to skip having to call the billing backend
        if visibility.is_public() {
            return Ok(());
        }
        Self::new(conn, biller, licensor, query_organization, visibility).await?;
        Ok(())
    }

    pub async fn check_for_project(
        conn: &mut DbConnection,
        biller: Option<&Biller>,
        licensor: &Licensor,
        query_project: &QueryProject,
        visibility: Visibility,
    ) -> Result<(), HttpError> {
        // Check if the project is public to skip having to call the billing backend
        if visibility.is_public() {
            return Ok(());
        }
        let query_organization = QueryOrganization::get(conn, query_project.organization_id)?;
        Self::new(conn, biller, licensor, &query_organization, visibility).await?;
        Ok(())
    }

    pub async fn check_usage(
        self,
        biller: Option<&Biller>,
        project: &QueryProject,
        usage: u32,
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
                    .record_metered_usage(&metered_plan_id, usage)
                    .await
                    .map_err(|e| {
                        issue_error(
                            StatusCode::BAD_REQUEST,
                            "Failed to record usage",
                            &format!("Failed to record usage ({usage}) for project ({project:?})."),
                            e,
                        )
                    })?;
            },
            Self::Licensed(LicenseUsage {
                entitlements,
                usage: prior_usage,
                level: _,
            }) => {
                if prior_usage + usage > entitlements.into() {
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

pub struct LicenseUsage {
    pub entitlements: Entitlements,
    pub usage: u32,
    pub level: PlanLevel,
}

impl LicenseUsage {
    pub fn get(
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
            level: token_data.claims.level(),
        }))
    }

    pub fn get_for_server(
        conn: &mut DbConnection,
        licensor: &Licensor,
        min_level: Option<PlanLevel>,
    ) -> Result<Vec<Self>, HttpError> {
        let min_level = min_level.unwrap_or_default();
        Ok(schema::organization::table
            .load::<QueryOrganization>(conn)
            .map_err(resource_not_found_err!(Organization))?
            .into_iter()
            .filter_map(|query_organization| {
                Self::get(conn, licensor, &query_organization)
                    .ok()
                    .flatten()
                    .and_then(|license_usage| {
                        (license_usage.level >= min_level).then_some(license_usage)
                    })
            })
            .collect())
    }
}
