#![cfg(feature = "plus")]

use bencher_billing::{Biller, CustomerId, HeldLicense, LicenseTerm, LicensedPlan};
use bencher_json::{
    DateTime, JsonPlan, Jwt, LicensedPlanId, MeteredPlanId, Priority,
    organization::plan::JsonReportLicense, project::Visibility,
};
use bencher_license::{LicenseClaims, LicenseError, Licensor};
use diesel::{BelongingToDsl as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;
use slog::Logger;

use crate::{
    ApiContext, actor_conn, auth_conn,
    context::{DbConnection, RateLimitingError},
    error::{
        BencherResource, issue_error, not_found_error, payment_required_error,
        resource_conflict_err, resource_not_found_err,
    },
    model::{
        organization::{OrganizationId, QueryOrganization, license::OrganizationLicense},
        project::QueryProject,
        user::{actor::ApiActor, auth::AuthUser},
    },
    schema::{self, plan as plan_table},
    write_transaction,
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

    /// The licensed plan, with the license that Bencher Cloud keeps for its Self-Hosted
    /// organization to copy, expired or not. When the subscription is active and pays for a
    /// later term than the license's, or for another count, a new license is signed and stored
    /// in its place first.
    ///
    /// A staff error in Stripe or a change of Bencher Cloud's signing key never fails the read.
    /// When an active subscription cannot pay for a new license, the license held stays and the
    /// read logs why. A new license that does not verify, as when the private key does not match
    /// the public key, is never stored, and the read logs why. A license that no longer verifies
    /// is signed again by the same rule, and one that is not signed again is left out of the
    /// plan, with a log line.
    pub async fn to_licensed_plan(
        &self,
        log: &Logger,
        context: &ApiContext,
        biller: &Biller,
    ) -> Result<Option<JsonPlan>, HttpError> {
        let Some(licensed_plan_id) = &self.licensed_plan else {
            return Ok(None);
        };

        let licensed_plan = biller
            .get_licensed_plan(licensed_plan_id)
            .await
            .map_err(resource_not_found_err!(Plan, self))?;

        let Some(held) = self.license.clone() else {
            return Err(issue_error(
                "Failed to find license for licensed plan",
                &format!(
                    "Failed to find license for plan ({self:?}) even though licensed plan exists ({licensed_plan:?}).",
                ),
                "Failed to find license for licensed plan",
            ));
        };
        let licensor = &context.licensor;
        let (held_claims, verified) = match licensor.read(&held) {
            Ok(claims) => (claims, true),
            Err(error) => {
                self.report(log, "The license held does not verify", &error);
                // Bencher Cloud wrote the license itself, so its claims still name the
                // Self-Hosted organization and the term it was signed for.
                match LicenseClaims::read_unverified(&held) {
                    Ok(claims) => (claims, false),
                    Err(error) => {
                        self.report(log, "The license held cannot be read", &error);
                        return Ok(Some(licensed_plan.plan));
                    },
                }
            },
        };
        let held_license = if verified {
            HeldLicense::Verified(&held_claims)
        } else {
            HeldLicense::Unverified(&held_claims)
        };

        let renewed = match licensed_plan.renewal(held_license) {
            Ok(Some(term)) => {
                let renewed = term
                    .sign(licensor, held_claims.organization())
                    .map_err(|e| {
                        self.license_error("Failed to sign license for licensed plan", e)
                    })?;
                match licensor.read(&renewed) {
                    Ok(_) => Some(self.store_renewal(context, renewed, term).await?),
                    Err(error) => {
                        self.report(
                            log,
                            "The new license does not verify, so Bencher Cloud stored none",
                            &error,
                        );
                        None
                    },
                }
            },
            Ok(None) => None,
            Err(error) => {
                let what = if verified {
                    "Bencher Cloud kept the license held and signed no new license"
                } else {
                    "Bencher Cloud signed no new license, and the plan shows none"
                };
                self.report(log, what, &error);
                None
            },
        };

        let LicensedPlan { mut plan, .. } = licensed_plan;
        plan.license = renewed
            .or_else(|| verified.then_some(held))
            .map(|license| licensor.into_json(license))
            .transpose()
            .map_err(|e| self.license_error("Failed to read license for licensed plan", e))?;
        Ok(Some(plan))
    }

    /// Store `renewed`, the license signed for `term`, in place of the license the plan holds,
    /// and return the license the plan holds after. The held license is read again under the
    /// write lock, so of two reads that renew at once, one stores its license and the other
    /// returns it, and a license for an earlier term never lands over a later one.
    async fn store_renewal(
        &self,
        context: &ApiContext,
        renewed: Jwt,
        term: LicenseTerm,
    ) -> Result<Jwt, HttpError> {
        let now = context.clock.now();
        let plan_query = schema::plan::table.filter(schema::plan::id.eq(self.id));
        write_transaction!(context, |conn| {
            let held: Option<Jwt> = plan_query.select(schema::plan::license).first(conn)?;
            if let Some(held) = held
                && context
                    .licensor
                    .read(&held)
                    .is_ok_and(|claims| !claims.replaced_by(term.series, term.term_end))
            {
                return Ok(held);
            }
            diesel::update(plan_query)
                .set((
                    schema::plan::license.eq(&renewed),
                    schema::plan::modified.eq(now),
                ))
                .execute(conn)?;
            diesel::QueryResult::Ok(renewed)
        })
        .map_err(resource_conflict_err!(Plan, self))
    }

    fn license_error(&self, title: &str, error: LicenseError) -> HttpError {
        issue_error(title, &format!("{title} for plan ({self:?})."), error)
    }

    /// Log `error`, which a read of this licensed plan works around, for staff.
    fn report<E>(&self, log: &Logger, what: &str, error: &E)
    where
        E: std::error::Error + 'static,
    {
        let id = &self.licensed_plan;
        slog::error!(log, "{what} for licensed plan ({id:?}): {error}");
        #[cfg(feature = "sentry")]
        sentry::capture_error(error);
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
    /// The plan row for a Self-Hosted deal: its subscription, and the license that Bencher
    /// Cloud signed for it. The organization that buys it holds no license of its own.
    pub fn licensed_plan(
        conn: &mut DbConnection,
        licensed_plan_id: LicensedPlanId,
        license: Jwt,
        query_organization: &QueryOrganization,
        now: DateTime,
    ) -> Result<Self, HttpError> {
        let insert_plan = Self {
            organization_id: query_organization.id,
            metered_plan: None,
            licensed_plan: Some(licensed_plan_id),
            license: Some(license),
            created: now,
            modified: now,
        };

        diesel::insert_into(schema::plan::table)
            .values(&insert_plan)
            .execute(conn)
            .map_err(resource_conflict_err!(Plan, insert_plan))?;

        Ok(insert_plan)
    }

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
    /// A valid Self-Hosted license, which is Enterprise.
    Licensed(OrganizationLicense),
    None,
}

#[derive(Debug, thiserror::Error)]
pub enum PlanKindError {
    #[error("No plan (subscription or license) found for organization ({uuid}) with private project", uuid = organization.uuid)]
    NoPlan { organization: QueryOrganization },
    #[error("The Bencher Plus Enterprise license for organization ({uuid}) is not valid, so it has no plan for a private project: {error}", uuid = organization.uuid)]
    InvalidLicense {
        organization: QueryOrganization,
        error: LicenseError,
    },
}

impl PlanKind {
    pub fn is_paid(&self) -> bool {
        match self {
            Self::Metered(_) | Self::Licensed(_) => true,
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
            return Ok(Self::Metered(metered_plan));
        }

        let license = OrganizationLicense::get(licensor, query_organization, context.clock.now());
        if let Ok(Some(license)) = license {
            Ok(Self::Licensed(license))
        } else if visibility.is_public() {
            // A license that is not valid reads as none, so it never blocks a public project.
            Ok(Self::None)
        } else {
            let organization = query_organization.clone();
            Err(payment_required_error(match license {
                Err(error) => PlanKindError::InvalidLicense {
                    organization,
                    error,
                },
                Ok(_) => PlanKindError::NoPlan { organization },
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

    /// After a report to an organization with a license: count its active Benchmark Series,
    /// raise the peak of the license's term, and return what the report response says about the
    /// license.
    /// A license never refuses a report here, and every other plan says nothing.
    pub async fn license_after_report(
        &self,
        log: &Logger,
        context: &ApiContext,
        organization_id: OrganizationId,
    ) -> Option<JsonReportLicense> {
        match self {
            Self::Licensed(license) => license.after_report(log, context, organization_id).await,
            Self::Metered(_) | Self::None => None,
        }
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

#[cfg(test)]
mod tests {
    use bencher_json::DateTime;

    use super::{MeteredPlan, PlanKind};
    use crate::model::organization::license::OrganizationLicense;

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

        let licensed = PlanKind::Licensed(OrganizationLicense::test());
        assert!(licensed.metered_series_billing().is_none());
        assert!(PlanKind::None.metered_series_billing().is_none());
    }

    // Fails if a valid license is not a paid plan, or no plan is.
    #[test]
    fn every_license_is_paid() {
        assert!(PlanKind::Licensed(OrganizationLicense::test()).is_paid());
        assert!(!PlanKind::None.is_paid());
    }
}
