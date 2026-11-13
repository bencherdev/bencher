#![cfg(feature = "plus")]

use bencher_json::{NonEmpty, OrganizationResourceId, OrganizationUuid};
use clap::{ArgGroup, Parser, Subcommand};

use crate::parser::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliOrganizationPlan {
    /// Create a subscription plan
    #[clap(alias = "add")]
    Create(CliPlanCreate),
    /// View a subscription plan
    #[clap(alias = "get")]
    View(CliPlanView),
    /// Update a subscription plan
    #[clap(alias = "edit")]
    Update(CliPlanUpdate),
    /// Delete a subscription plan
    #[clap(alias = "rm")]
    Delete(CliPlanDelete),
}

#[derive(Parser, Debug)]
#[clap(group(
    ArgGroup::new("plan")
        .multiple(false)
        .required(true)
        .args(&["checkout", "subscription"]),
))]
pub struct CliPlanCreate {
    /// Organization slug or UUID
    pub organization: OrganizationResourceId,

    /// Checkout session ID of a completed self-serve checkout
    #[clap(long)]
    pub checkout: Option<NonEmpty>,

    /// Subscription ID to attach (server admin only)
    #[clap(long)]
    pub subscription: Option<NonEmpty>,

    /// Self-Hosted organization UUID that the subscription buys a license for, signed by Bencher Cloud (server admin only)
    #[clap(long, requires = "subscription", conflicts_with = "checkout")]
    pub self_hosted: Option<OrganizationUuid>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliPlanView {
    /// Organization slug or UUID
    pub organization: OrganizationResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
#[clap(group(
    ArgGroup::new("cancellation")
        .multiple(false)
        .required(true)
        .args(&["cancel", "resume"]),
))]
pub struct CliPlanUpdate {
    /// Organization slug or UUID
    pub organization: OrganizationResourceId,

    /// Schedule the metered subscription to cancel at the end of the current billing period
    #[clap(long)]
    pub cancel: bool,

    /// Resume a metered subscription scheduled to cancel at the end of the current billing period
    #[clap(long)]
    pub resume: bool,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliPlanDelete {
    /// Organization slug or UUID
    pub organization: OrganizationResourceId,

    /// Skip sending to remote provider
    #[clap(long)]
    pub skip_remote: bool,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[cfg(test)]
mod tests {
    use clap::{Parser as _, error::ErrorKind};

    use crate::parser::CliBencher;

    fn plan_create(args: &[&str]) -> Result<CliBencher, clap::Error> {
        CliBencher::try_parse_from(
            ["bencher", "plan", "create", "my-org"]
                .iter()
                .chain(args)
                .copied(),
        )
    }

    const SELF_HOSTED: &str = "00000000-0000-4000-8000-000000000001";

    // Fails if a checkout return or a subscription attach does not parse on its own, or an
    // attach for a Self-Hosted organization does not parse.
    #[test]
    fn plan_create_takes_checkout_or_subscription() {
        plan_create(&["--checkout", "cs_test"]).unwrap();
        plan_create(&["--subscription", "sub_test"]).unwrap();
        plan_create(&["--subscription", "sub_test", "--self-hosted", SELF_HOSTED]).unwrap();
    }

    // Fails if a Self-Hosted organization is taken with a checkout return or without a
    // subscription, or one that is not a UUID is taken.
    #[test]
    fn plan_create_takes_self_hosted_with_a_subscription_only() {
        assert_eq!(
            plan_create(&["--checkout", "cs_test", "--self-hosted", SELF_HOSTED])
                .unwrap_err()
                .kind(),
            ErrorKind::ArgumentConflict
        );
        assert_eq!(
            plan_create(&["--self-hosted", SELF_HOSTED])
                .unwrap_err()
                .kind(),
            ErrorKind::MissingRequiredArgument
        );
        assert_eq!(
            plan_create(&["--subscription", "sub_test", "--self-hosted", "acme"])
                .unwrap_err()
                .kind(),
            ErrorKind::ValueValidation
        );
    }

    // Fails if a create that gives both a checkout and a subscription, or neither, parses.
    #[test]
    fn plan_create_takes_exactly_one() {
        assert_eq!(
            plan_create(&["--checkout", "cs_test", "--subscription", "sub_test"])
                .unwrap_err()
                .kind(),
            ErrorKind::ArgumentConflict
        );
        assert_eq!(
            plan_create(&[]).unwrap_err().kind(),
            ErrorKind::MissingRequiredArgument
        );
    }

    // Fails if a retired create option still parses.
    #[test]
    fn plan_create_retired_options() {
        for retired in [
            ["--level", "pro"].as_slice(),
            &["--entitlements", "1000"],
            &["--skip-remote"],
        ] {
            let args: Vec<&str> = ["--checkout", "cs_test"]
                .iter()
                .chain(retired)
                .copied()
                .collect();
            assert_eq!(
                plan_create(&args).unwrap_err().kind(),
                ErrorKind::UnknownArgument,
                "{retired:?}"
            );
        }
    }
}
