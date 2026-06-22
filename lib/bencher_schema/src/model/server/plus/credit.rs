#![cfg(feature = "plus")]

use std::cmp;
use std::path::PathBuf;

use bencher_billing::Biller;
use bencher_json::PlanStatus;
use chrono::{Duration, NaiveTime, Utc};
use diesel::Connection as _;
use slog::Logger;

use crate::context::DbConnection;
use crate::model::organization::plan::QueryPlan;

use super::configure_standalone_connection;

/// Time of day (UTC) at which the daily credit sweep runs. A fixed time so the
/// sweep happens at the same time every day rather than drifting with server
/// restarts.
const CREDIT_SWEEP_TIME: NaiveTime = NaiveTime::MIN;

/// How long to wait before retrying a failed database connection during the daily
/// credit sweep, so a transient failure does not skip the whole day's run.
const CREDIT_SWEEP_RETRY_DELAY: std::time::Duration = std::time::Duration::from_mins(1);

/// Maximum number of connection attempts for a single daily sweep before giving
/// up until the next scheduled run.
const CREDIT_SWEEP_RETRY_LIMIT: usize = 5;

/// Daily background sweep (Bencher Cloud only) that keeps each metered
/// subscription's included usage credit granted for the current billing period
/// and prunes the local plan row once a subscription has fully lapsed. This keeps
/// credit granting off the report-ingestion hot path, free of billing-side Stripe
/// calls.
pub fn spawn_credit_grants(log: Logger, db_path: PathBuf, busy_timeout: u32, biller: Biller) {
    tokio::spawn(async move {
        loop {
            // Sleep until the next occurrence of the daily sweep time (UTC) so the
            // sweep runs at a fixed time of day, not relative to server start.
            let now = Utc::now().naive_utc().time();
            let sleep_time = match now.cmp(&CREDIT_SWEEP_TIME) {
                cmp::Ordering::Less => CREDIT_SWEEP_TIME - now,
                cmp::Ordering::Equal => Duration::days(1),
                cmp::Ordering::Greater => Duration::days(1) - (now - CREDIT_SWEEP_TIME),
            }
            .to_std()
            .unwrap_or(std::time::Duration::from_hours(24));
            tokio::time::sleep(sleep_time).await;

            // Open a configured connection for this sweep, retrying briefly on a
            // transient failure rather than skipping the whole day's run.
            let mut attempt = 0;
            let conn = loop {
                attempt += 1;
                match DbConnection::establish(db_path.to_string_lossy().as_ref()) {
                    Ok(mut conn) => {
                        match configure_standalone_connection(&mut conn, busy_timeout) {
                            Ok(()) => break Some(conn),
                            Err(e) => slog::error!(
                                log,
                                "Failed to configure database connection PRAGMAs for credit sweep: {e}"
                            ),
                        }
                    },
                    Err(e) => slog::error!(
                        log,
                        "Failed to establish database connection for credit sweep: {e}"
                    ),
                }
                if attempt >= CREDIT_SWEEP_RETRY_LIMIT {
                    break None;
                }
                tokio::time::sleep(CREDIT_SWEEP_RETRY_DELAY).await;
            };
            let Some(mut conn) = conn else {
                slog::error!(
                    log,
                    "Giving up on credit sweep until the next scheduled run after {CREDIT_SWEEP_RETRY_LIMIT} failed connection attempts"
                );
                continue;
            };

            let plans = match QueryPlan::all_metered(&mut conn) {
                Ok(plans) => plans,
                Err(e) => {
                    slog::error!(log, "Failed to load metered plans for credit sweep: {e}");
                    continue;
                },
            };

            for plan in plans {
                let Some(metered_plan_id) = plan.metered_plan.clone() else {
                    continue;
                };
                let plan_id = plan.id;
                match biller.get_metered_plan_status(&metered_plan_id).await {
                    Ok((status, _, _)) => match credit_sweep_action(status) {
                        // Active (or trialing): ensure this period's included credit.
                        // Idempotent, and a no-op for metered plans without a base fee.
                        CreditSweepAction::EnsureCredit => {
                            if let Err(e) = biller.ensure_period_credit(&metered_plan_id).await {
                                slog::warn!(
                                    log,
                                    "Failed to ensure period credit for {metered_plan_id}: {e}"
                                );
                                #[cfg(feature = "sentry")]
                                sentry::capture_error(&e);
                            }
                        },
                        // Terminal: prune the local plan row so the org reads as Free
                        // and we stop querying a dead subscription.
                        CreditSweepAction::Prune => {
                            if let Err(e) = QueryPlan::delete(&mut conn, plan_id) {
                                slog::warn!(
                                    log,
                                    "Failed to prune lapsed plan {metered_plan_id}: {e}"
                                );
                            }
                        },
                        // Recoverable/transient: leave the plan row intact so the
                        // subscription can recover; grant nothing this run.
                        CreditSweepAction::Skip => {},
                    },
                    Err(e) => {
                        slog::warn!(log, "Failed to fetch status for {metered_plan_id}: {e}");
                        #[cfg(feature = "sentry")]
                        sentry::capture_error(&e);
                    },
                }
            }
        }
    });
}

/// The action the daily sweep takes for a metered plan, derived from its
/// subscription status. Extracted as a pure function so the branching is
/// unit-testable without Stripe or a database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CreditSweepAction {
    /// Active or trialing: ensure this period's included usage credit.
    EnsureCredit,
    /// Terminally dead (canceled or incomplete-expired): prune the local plan row.
    Prune,
    /// Recoverable or transient (incomplete, past due, paused, unpaid): leave the
    /// plan row in place and grant nothing; the subscription may still recover.
    Skip,
}

/// Decide the sweep action from the subscription status. Only terminal states are
/// pruned: deleting the local plan row severs the only org-to-subscription link
/// (no webhook re-creates it), so a subscription that is merely retrying payment
/// (`PastDue`/`Unpaid`), awaiting its first charge (`Incomplete`), or paused must
/// be left intact to recover. The exhaustive match forces a deliberate choice if a
/// new `PlanStatus` is added.
fn credit_sweep_action(status: PlanStatus) -> CreditSweepAction {
    match status {
        PlanStatus::Active | PlanStatus::Trialing => CreditSweepAction::EnsureCredit,
        PlanStatus::Canceled | PlanStatus::IncompleteExpired => CreditSweepAction::Prune,
        PlanStatus::Incomplete | PlanStatus::PastDue | PlanStatus::Paused | PlanStatus::Unpaid => {
            CreditSweepAction::Skip
        },
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::PlanStatus;

    use super::{CreditSweepAction, credit_sweep_action};

    #[test]
    fn active_statuses_ensure_credit() {
        for status in [PlanStatus::Active, PlanStatus::Trialing] {
            assert_eq!(credit_sweep_action(status), CreditSweepAction::EnsureCredit);
        }
    }

    #[test]
    fn terminal_statuses_prune() {
        for status in [PlanStatus::Canceled, PlanStatus::IncompleteExpired] {
            assert_eq!(credit_sweep_action(status), CreditSweepAction::Prune);
        }
    }

    #[test]
    fn recoverable_statuses_skip() {
        for status in [
            PlanStatus::Incomplete,
            PlanStatus::PastDue,
            PlanStatus::Paused,
            PlanStatus::Unpaid,
        ] {
            assert_eq!(credit_sweep_action(status), CreditSweepAction::Skip);
        }
    }
}
