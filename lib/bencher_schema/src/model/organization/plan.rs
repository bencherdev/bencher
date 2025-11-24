#![cfg(feature = "plus")]

use bencher_billing::Biller;
use bencher_json::{
    DateTime, Entitlements, JsonPlan, Jwt, LicensedPlanId, MeteredPlanId, OrganizationUuid,
    PlanLevel, project::Visibility,
};
use bencher_license::Licensor;
use diesel::{BelongingToDsl as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;
use tokio::sync::Mutex;

use crate::{
    ApiContext, conn_lock, connection_lock,
    context::{DbConnection, RateLimitingError},
    error::{
        issue_error, not_found_error, payment_required_error, resource_conflict_err,
        resource_not_found_err,
    },
    model::{
        organization::{OrganizationId, QueryOrganization, UpdateOrganization},
        project::{QueryProject, metric::QueryMetric},
    },
    schema::{self, plan as plan_table},
};

crate::macros::typed_id::typed_id!(PlanId);

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
        context: &ApiContext,
        biller: Option<&Biller>,
        query_organization: &QueryOrganization,
    ) -> Result<Option<MeteredPlanId>, HttpError> {
        let Some(biller) = biller else {
            return Ok(None);
        };

        let Ok(query_plan) =
            Self::belonging_to(&query_organization).first::<QueryPlan>(conn_lock!(context))
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

pub enum PlanKind {
    Metered(MeteredPlanId),
    Licensed(LicenseUsage),
    None,
}

#[derive(Debug, thiserror::Error)]
pub enum PlanKindError {
    #[error("Organization ({uuid}) has an inactive metered plan ({metered_plan_id})", uuid = organization.uuid)]
    InactiveMeteredPlan {
        organization: QueryOrganization,
        metered_plan_id: MeteredPlanId,
    },
    #[error("License usage exceeded for organization ({uuid}). {usage} > {entitlements}", uuid = organization.uuid)]
    LicensePlanOverage {
        organization: QueryOrganization,
        entitlements: Entitlements,
        usage: u32,
    },
    #[error("No plan (subscription or license) found for organization ({uuid}) with private project", uuid = organization.uuid)]
    NoPlan { organization: QueryOrganization },
    #[error("No Biller has been configured for the server")]
    NoBiller,
    #[error("License usage exceeded for project ({uuid}). {prior_usage} + {usage} > {entitlements}", uuid = project.uuid)]
    Overage {
        project: QueryProject,
        entitlements: Entitlements,
        prior_usage: u32,
        usage: u32,
    },
}

impl PlanKind {
    async fn new(
        context: &ApiContext,
        biller: Option<&Biller>,
        licensor: &Licensor,
        query_organization: &QueryOrganization,
        visibility: Visibility,
    ) -> Result<Self, HttpError> {
        if let Some(metered_plan_id) =
            QueryPlan::get_active_metered_plan(context, biller, query_organization).await?
        {
            Ok(Self::Metered(metered_plan_id))
        } else if let Some(license_usage) =
            LicenseUsage::get(&context.database.connection, licensor, query_organization).await?
        {
            if license_usage.usage < license_usage.entitlements.into() {
                Ok(Self::Licensed(license_usage))
            } else {
                Err(payment_required_error(PlanKindError::LicensePlanOverage {
                    organization: query_organization.clone(),
                    entitlements: license_usage.entitlements,
                    usage: license_usage.usage,
                }))
            }
        } else if visibility.is_public() {
            let is_claimed = query_organization.is_claimed(conn_lock!(context))?;
            let window_usage = query_organization.window_usage(context).await?;

            context
                .rate_limiting
                .check_claimable_limit(
                    is_claimed,
                    window_usage,
                    |rate_limit| RateLimitingError::UnclaimedOrganization {
                        organization: query_organization.clone(),
                        rate_limit,
                    },
                    |rate_limit| RateLimitingError::ClaimedOrganization {
                        organization: query_organization.clone(),
                        rate_limit,
                    },
                )
                .map(|()| Self::None)
        } else {
            Err(payment_required_error(PlanKindError::NoPlan {
                organization: query_organization.clone(),
            }))
        }
    }

    pub async fn new_for_project(
        context: &ApiContext,
        biller: Option<&Biller>,
        licensor: &Licensor,
        project: &QueryProject,
    ) -> Result<Self, HttpError> {
        let query_organization = project.organization(conn_lock!(context))?;
        Self::new(
            context,
            biller,
            licensor,
            &query_organization,
            project.visibility,
        )
        .await
    }

    pub async fn check_for_organization(
        context: &ApiContext,
        biller: Option<&Biller>,
        licensor: &Licensor,
        query_organization: &QueryOrganization,
        visibility: Visibility,
    ) -> Result<(), HttpError> {
        // Check if the project is public to skip having to call the billing backend
        if visibility.is_public() {
            return Ok(());
        }
        Self::new(context, biller, licensor, query_organization, visibility).await?;
        Ok(())
    }

    pub async fn check_for_project(
        context: &ApiContext,
        biller: Option<&Biller>,
        licensor: &Licensor,
        query_project: &QueryProject,
        visibility: Visibility,
    ) -> Result<(), HttpError> {
        // Check if the project is public to skip having to call the billing backend
        if visibility.is_public() {
            return Ok(());
        }
        let query_organization = query_project.organization(conn_lock!(context))?;
        Self::new(context, biller, licensor, &query_organization, visibility).await?;
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
    pub async fn get(
        db_connection: &Mutex<DbConnection>,
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

        let usage = QueryMetric::usage(
            connection_lock!(db_connection),
            query_organization.id,
            start_time,
            end_time,
        )?;
        let entitlements = licensor
            .validate_usage(&token_data.claims, usage)
            .map_err(payment_required_error)?;

        Ok(Some(LicenseUsage {
            entitlements,
            usage,
            level: token_data.claims.level(),
        }))
    }

    pub async fn get_for_server(
        db_connection: &Mutex<DbConnection>,
        licensor: &Licensor,
        min_level: Option<PlanLevel>,
    ) -> Result<Vec<Self>, HttpError> {
        let min_level = min_level.unwrap_or_default();
        let organizations = schema::organization::table
            .load::<QueryOrganization>(connection_lock!(db_connection))
            .map_err(resource_not_found_err!(Organization))?;
        let mut license_usages = Vec::new();
        for query_organization in organizations {
            if let Ok(Some(license_usage)) =
                Self::get(db_connection, licensor, &query_organization).await
            {
                if license_usage.level >= min_level {
                    license_usages.push(license_usage);
                }
            }
        }
        Ok(license_usages)
    }
}
