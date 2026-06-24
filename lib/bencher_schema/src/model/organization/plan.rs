#![cfg(feature = "plus")]

use bencher_billing::{Biller, CustomerId};
use bencher_json::{
    DateTime, Entitlements, JsonPlan, Jwt, LicensedPlanId, MeteredPlanId, OrganizationUuid,
    PlanLevel, Priority, project::Visibility,
};
use bencher_license::Licensor;
use diesel::{BelongingToDsl as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::{
    ApiContext, actor_conn, auth_conn,
    context::{DbConnection, RateLimitingError},
    error::{
        issue_error, not_found_error, payment_required_error, resource_conflict_err,
        resource_not_found_err,
    },
    model::{
        organization::{OrganizationId, QueryOrganization, UpdateOrganization},
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

        let Ok(query_plan) = Self::belonging_to(&query_organization)
            .first::<QueryPlan>(actor_conn!(context, api_actor))
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
                level: billing.level,
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

/// An active metered (Stripe) subscription's billing context, carried by
/// [`PlanKind::Metered`]: who to bill (`customer_id`), the tier (`level`), and the
/// current billing period (the active-series count window for the post-report Pro
/// series push).
pub struct MeteredPlan {
    pub customer_id: CustomerId,
    pub level: PlanLevel,
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
    pub fn priority(&self, is_claimed: bool) -> Priority {
        if !is_claimed {
            return Priority::Unclaimed;
        }
        match self {
            Self::None => Priority::Free,
            Self::Metered(_) => Priority::Plus,
            Self::Licensed(license_usage) => match license_usage.level {
                PlanLevel::Free => Priority::Free,
                PlanLevel::Pro | PlanLevel::Team | PlanLevel::Enterprise => Priority::Plus,
            },
        }
    }

    /// For a Pro metered plan, the Stripe customer and current billing period needed to
    /// post the post-report active-series usage. `None` for any non-Pro plan, so only
    /// Pro triggers a series post. Pairs with [`metered_bills_active_series`].
    pub fn metered_series_billing(&self) -> Option<(CustomerId, DateTime, DateTime)> {
        match self {
            Self::Metered(MeteredPlan {
                customer_id,
                level,
                current_period_start,
                current_period_end,
            }) if metered_bills_active_series(*level) => Some((
                customer_id.clone(),
                *current_period_start,
                *current_period_end,
            )),
            Self::Metered(_) | Self::Licensed(_) | Self::None => None,
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
            let is_claimed = query_organization.is_claimed(actor_conn!(context, api_actor))?;
            let window_usage = query_organization.window_usage(context, api_actor).await?;

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
        api_actor: &ApiActor,
    ) -> Result<Self, HttpError> {
        let query_organization = project.organization(actor_conn!(context, api_actor))?;
        Self::new(
            context,
            biller,
            licensor,
            api_actor,
            &query_organization,
            project.visibility,
        )
        .await
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

    pub async fn check_usage(
        self,
        biller: Option<&Biller>,
        project: &QueryProject,
        usage: u32,
    ) -> Result<(), HttpError> {
        match self {
            Self::Metered(MeteredPlan {
                customer_id, level, ..
            }) => {
                // Pro bills on active series via its tiered price (posted after each
                // report), not per-metric, so it records no metrics usage. Legacy Team
                // (and metered Enterprise) plans bill all metrics on the `metrics`
                // meter. Bare metal runner time is metered separately via the runner
                // channel.
                if metered_bills_active_series(level) {
                    return Ok(());
                }
                let Some(biller) = biller else {
                    return Err(issue_error(
                        "No Biller when checking usage",
                        "Failed to find Biller in Bencher Cloud when checking usage.",
                        PlanKindError::NoBiller,
                    ));
                };
                if let Err(e) = biller.record_metrics_usage(&customer_id, usage).await {
                    #[cfg(feature = "otel")]
                    bencher_otel::ApiMeter::increment(
                        bencher_otel::ApiCounter::MetricsBilledFailed,
                    );
                    return Err(issue_error(
                        "Failed to record usage",
                        &format!("Failed to record usage ({usage}) for project ({project:?})."),
                        e,
                    ));
                }
                #[cfg(feature = "otel")]
                bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::MetricsBilled);
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

/// Whether a metered (Stripe) subscription counts *public* project metrics in the usage
/// estimate shown by the usage endpoint.
///
/// Only legacy Team (and metered Enterprise, which the price heuristic resolves to
/// `Team`) plans bill public metrics, so their estimate counts all metrics; Pro's
/// estimate counts only private metrics. This governs only the metrics figure shown for
/// metered plans. Pro itself now bills on active series, not metrics (see
/// [`metered_bills_active_series`]).
pub fn metered_bills_public_metrics(level: PlanLevel) -> bool {
    // Matched exhaustively so a new `PlanLevel` variant forces a decision here.
    match level {
        PlanLevel::Free | PlanLevel::Pro => false,
        PlanLevel::Team | PlanLevel::Enterprise => true,
    }
}

/// Whether a metered (Stripe) subscription is billed on the active-series meter.
///
/// Active-series billing is the Pro plan only: Pro's tiered price bills the base fee and
/// the per-series step-ups on the `active_series` meter, and Pro records no per-metric
/// `metrics` usage (see [`PlanKind::check_usage`]). Legacy Team and metered Enterprise
/// plans stay on the `metrics` meter and have no series usage recorded.
///
/// Also gates the post-report series push: after a Pro report we count the
/// organization's active series and post the period-to-date total to the
/// `active_series` meter.
///
/// Matched exhaustively so a new `PlanLevel` variant forces a decision here.
pub fn metered_bills_active_series(level: PlanLevel) -> bool {
    match level {
        PlanLevel::Pro => true,
        PlanLevel::Free | PlanLevel::Team | PlanLevel::Enterprise => false,
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
    use bencher_json::{DateTime, PlanLevel};

    use super::{MeteredPlan, PlanKind, metered_bills_active_series, metered_bills_public_metrics};

    #[test]
    fn does_not_bill_public_metrics() {
        assert!(!metered_bills_public_metrics(PlanLevel::Free));
        // Pro is the only metered tier whose public metrics are free.
        assert!(!metered_bills_public_metrics(PlanLevel::Pro));
    }

    #[test]
    fn bills_public_metrics() {
        // Legacy Team and metered Enterprise are billed for public metrics. Free is
        // included for completeness even though metered plans only resolve to
        // Pro/Team via the Pro-price heuristic.
        assert!(metered_bills_public_metrics(PlanLevel::Team));
        assert!(metered_bills_public_metrics(PlanLevel::Enterprise));
    }

    #[test]
    fn bills_active_series_pro_only() {
        // Active-series billing is the Pro plan only.
        assert!(metered_bills_active_series(PlanLevel::Pro));
        assert!(!metered_bills_active_series(PlanLevel::Free));
        assert!(!metered_bills_active_series(PlanLevel::Team));
        assert!(!metered_bills_active_series(PlanLevel::Enterprise));
    }

    #[test]
    fn metered_series_billing_pro_only() {
        let metered = |level| {
            PlanKind::Metered(MeteredPlan {
                customer_id: "cus_test".into(),
                level,
                current_period_start: DateTime::TEST,
                current_period_end: DateTime::TEST,
            })
        };
        // Only Pro resolves a post-report series-billing context; other metered tiers
        // and the non-metered kinds do not.
        assert!(metered(PlanLevel::Pro).metered_series_billing().is_some());
        assert!(metered(PlanLevel::Team).metered_series_billing().is_none());
        assert!(
            metered(PlanLevel::Enterprise)
                .metered_series_billing()
                .is_none()
        );
        assert!(PlanKind::None.metered_series_billing().is_none());
    }
}
