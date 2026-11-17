#![cfg(feature = "plus")]

use bencher_billing::{Biller, CustomerId};
use bencher_json::{
    DateTime, Entitlements, JsonPlan, Jwt, LicensedPlanId, MeteredPlanId, PlanLevel, Priority,
    project::Visibility,
};
use bencher_license::Licensor;
use diesel::{BelongingToDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::{
    ApiContext, actor_conn, auth_conn,
    context::{DbConnection, RateLimitingError},
    error::{
        BencherResource, issue_error, not_found_error, payment_required_error,
        resource_conflict_err, resource_not_found_err,
    },
    model::{
        organization::{OrganizationId, QueryOrganization},
        project::{QueryProject, metric::QueryMetric},
        user::{actor::ApiActor, auth::AuthUser},
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
            .into_json(license)
            .map_err(payment_required_error)?;
        json_plan.license = Some(json_license);

        Ok(Some(json_plan))
    }

    pub async fn get_active_metered_plan(
        context: &ApiContext,
        biller: Option<&Biller>,
        api_actor: &ApiActor,
        query_organization: &QueryOrganization,
    ) -> Result<Option<MeteredPlan>, HttpError> {
        let Some(biller) = biller else {
            return Ok(None);
        };

        let Ok(query_plan) =
            Self::belonging_to(&query_organization).first::<Self>(actor_conn!(context, api_actor))
        else {
            return Ok(None);
        };

        let Some(metered_plan_id) = query_plan.metered_plan.clone() else {
            return Ok(None);
        };

        let billing = biller
            .get_metered_plan_billing(&metered_plan_id)
            .await
            .map_err(not_found_error)?;

        // A canceled/lapsed (inactive) subscription gracefully downgrades to
        // Free: return `None` so the caller falls through to the public/no-plan
        // logic instead of hard-erroring.
        if billing.status.is_active() {
            Ok(Some(MeteredPlan {
                customer_id: billing.customer_id,
                current_period_start: billing.current_period_start,
                current_period_end: billing.current_period_end,
            }))
        } else {
            Ok(None)
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
        let insert_plan = Self {
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
}

/// An active metered (Stripe) subscription's billing context, carried by
/// [`PlanKind::Metered`].
///
/// It holds who to bill (`customer_id`) and the current billing period (the active-series
/// count window for the post-report series push).
pub struct MeteredPlan {
    pub customer_id: CustomerId,
    pub current_period_start: DateTime,
    pub current_period_end: DateTime,
}

pub enum PlanKind {
    Metered(MeteredPlan),
    Licensed(LicenseUsage),
    None,
}

#[derive(Debug, thiserror::Error)]
pub enum PlanKindError {
    #[error("License usage exceeded for organization ({uuid}). {usage} > {entitlements}", uuid = organization.uuid)]
    LicensePlanOverage {
        organization: QueryOrganization,
        entitlements: Entitlements,
        usage: u32,
    },
    #[error("No plan (subscription or license) found for organization ({uuid}) with private project", uuid = organization.uuid)]
    NoPlan { organization: QueryOrganization },
    #[error("License usage exceeded for project ({uuid}). {prior_usage} + {usage} > {entitlements}", uuid = project.uuid)]
    Overage {
        project: QueryProject,
        entitlements: Entitlements,
        prior_usage: u32,
        usage: u32,
    },
}

impl PlanKind {
    pub fn is_paid(&self) -> bool {
        match self {
            Self::Metered(_) => true,
            Self::Licensed(license_usage) => match license_usage.level {
                PlanLevel::Free => false,
                PlanLevel::Pro | PlanLevel::Enterprise => true,
            },
            Self::None => false,
        }
    }

    pub fn priority(&self, is_claimed: bool) -> Priority {
        if !is_claimed {
            return Priority::Unclaimed;
        }
        if self.is_paid() {
            Priority::Plus
        } else {
            Priority::Free
        }
    }

    /// For a metered plan, the Stripe customer and current billing period needed to post
    /// the post-report active-series usage. `None` for any other plan.
    pub fn metered_series_billing(&self) -> Option<(CustomerId, DateTime, DateTime)> {
        match self {
            Self::Metered(MeteredPlan {
                customer_id,
                current_period_start,
                current_period_end,
            }) => Some((
                customer_id.clone(),
                *current_period_start,
                *current_period_end,
            )),
            Self::Licensed(_) | Self::None => None,
        }
    }

    async fn new(
        context: &ApiContext,
        biller: Option<&Biller>,
        licensor: &Licensor,
        api_actor: &ApiActor,
        query_organization: &QueryOrganization,
        visibility: Visibility,
    ) -> Result<Self, HttpError> {
        if let Some(metered_plan) =
            QueryPlan::get_active_metered_plan(context, biller, api_actor, query_organization)
                .await?
        {
            Ok(Self::Metered(metered_plan))
        } else if let Some(license_usage) = LicenseUsage::get(
            actor_conn!(context, api_actor),
            licensor,
            query_organization,
        )? {
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
            Ok(Self::None)
        } else {
            Err(payment_required_error(PlanKindError::NoPlan {
                organization: query_organization.clone(),
            }))
        }
    }

    async fn check_claimable_limit(
        context: &ApiContext,
        api_actor: &ApiActor,
        query_organization: &QueryOrganization,
    ) -> Result<(), HttpError> {
        let is_claimed = query_organization.is_claimed(actor_conn!(context, api_actor))?;
        let window_usage = query_organization.window_usage(context, api_actor).await?;
        context.rate_limiting.check_claimable_limit(
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
    }

    async fn check_plus_limit(
        context: &ApiContext,
        api_actor: &ApiActor,
        query_organization: &QueryOrganization,
    ) -> Result<(), HttpError> {
        let window_usage = query_organization.window_usage(context, api_actor).await?;
        context
            .rate_limiting
            .check_plus_limit(window_usage, |rate_limit| RateLimitingError::Organization {
                organization: query_organization.clone(),
                resource: BencherResource::Metric,
                rate_limit,
            })
    }

    /// The plan for a new report or run, held to its organization's daily metrics limit.
    pub async fn new_for_project(
        context: &ApiContext,
        biller: Option<&Biller>,
        licensor: &Licensor,
        project: &QueryProject,
        api_actor: &ApiActor,
    ) -> Result<Self, HttpError> {
        let query_organization = project.organization(actor_conn!(context, api_actor))?;
        let plan_kind = Self::new(
            context,
            biller,
            licensor,
            api_actor,
            &query_organization,
            project.visibility,
        )
        .await?;
        if plan_kind.is_paid() {
            Self::check_plus_limit(context, api_actor, &query_organization).await?;
        } else {
            Self::check_claimable_limit(context, api_actor, &query_organization).await?;
        }
        Ok(plan_kind)
    }

    /// The plan for the results of a job that has already run, which a paid plan's
    /// daily metrics limit does not refuse.
    pub async fn new_for_job_results(
        context: &ApiContext,
        project: &QueryProject,
        api_actor: &ApiActor,
    ) -> Result<Self, HttpError> {
        let query_organization = project.organization(actor_conn!(context, api_actor))?;
        let plan_kind = Self::new(
            context,
            context.biller.as_ref(),
            &context.licensor,
            api_actor,
            &query_organization,
            project.visibility,
        )
        .await?;
        if !plan_kind.is_paid() {
            Self::check_claimable_limit(context, api_actor, &query_organization).await?;
        }
        Ok(plan_kind)
    }

    pub async fn check_for_organization(
        context: &ApiContext,
        biller: Option<&Biller>,
        licensor: &Licensor,
        auth_user: &AuthUser,
        query_organization: &QueryOrganization,
        visibility: Visibility,
    ) -> Result<(), HttpError> {
        // Check if the project is public to skip having to call the billing backend
        if visibility.is_public() {
            return Ok(());
        }
        Self::new(
            context,
            biller,
            licensor,
            &auth_user.clone().into(),
            query_organization,
            visibility,
        )
        .await?;
        Ok(())
    }

    pub async fn check_for_project(
        context: &ApiContext,
        biller: Option<&Biller>,
        licensor: &Licensor,
        auth_user: AuthUser,
        query_project: &QueryProject,
        visibility: Visibility,
    ) -> Result<(), HttpError> {
        // Check if the project is public to skip having to call the billing backend
        if visibility.is_public() {
            return Ok(());
        }
        let query_organization = query_project.organization(auth_conn!(context))?;
        Self::new(
            context,
            biller,
            licensor,
            &auth_user.into(),
            &query_organization,
            visibility,
        )
        .await?;
        Ok(())
    }

    /// Hold a licensed plan to its entitlements. A metered plan bills on active series,
    /// which the report posts once its metrics are committed, so it records nothing here.
    pub fn check_usage(self, project: &QueryProject, usage: u32) -> Result<(), HttpError> {
        match self {
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
            Self::Metered(_) | Self::None => {},
        }

        Ok(())
    }
}

#[cfg(feature = "otel")]
impl From<&PlanKind> for bencher_otel::PlanKind {
    fn from(plan_kind: &PlanKind) -> Self {
        match plan_kind {
            PlanKind::Metered(_) => Self::Metered,
            PlanKind::Licensed(_) => Self::Licensed,
            PlanKind::None => Self::None,
        }
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
    ) -> Result<Option<Self>, HttpError> {
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

        Ok(Some(Self {
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

        // Intentionally includes soft-deleted organizations for server admin license usage
        Ok(schema::organization::table
            .load::<QueryOrganization>(conn)
            .map_err(resource_not_found_err!(Organization))?
            .iter()
            .filter_map(|query_organization| {
                if let Ok(Some(license_usage)) = Self::get(conn, licensor, query_organization)
                    && license_usage.level >= min_level
                {
                    Some(license_usage)
                } else {
                    None
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::{DateTime, Entitlements, PlanLevel};

    use super::{LicenseUsage, MeteredPlan, PlanKind};

    // Fails if a metered plan does not post its active series for its own customer and
    // period, or any other plan does.
    #[test]
    fn metered_series_billing_every_metered_plan() {
        let period_end = DateTime::try_from(DateTime::TEST.timestamp() + 1).unwrap();
        let metered = PlanKind::Metered(MeteredPlan {
            customer_id: "cus_test".into(),
            current_period_start: DateTime::TEST,
            current_period_end: period_end,
        });
        assert_eq!(
            metered.metered_series_billing(),
            Some(("cus_test".into(), DateTime::TEST, period_end)),
        );

        let licensed = PlanKind::Licensed(LicenseUsage {
            entitlements: Entitlements::MIN,
            usage: 0,
            level: PlanLevel::Enterprise,
        });
        assert!(licensed.metered_series_billing().is_none());
        assert!(PlanKind::None.metered_series_billing().is_none());
    }
}
