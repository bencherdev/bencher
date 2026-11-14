use bencher_client::types::{JsonNewPlan, JsonPlanCheckout, JsonPlanSubscription};
use bencher_json::{NonEmpty, OrganizationResourceId};

use crate::{
    CliError,
    bencher::{backend::AuthBackend, sub::SubCmd},
    parser::organization::plan::CliPlanCreate,
};

#[derive(Debug, Clone)]
pub struct Create {
    pub organization: OrganizationResourceId,
    pub plan: JsonNewPlan,
    pub backend: AuthBackend,
}

impl TryFrom<CliPlanCreate> for Create {
    type Error = CliError;

    fn try_from(create: CliPlanCreate) -> Result<Self, Self::Error> {
        let CliPlanCreate {
            organization,
            checkout,
            subscription,
            backend,
        } = create;
        Ok(Self {
            organization,
            plan: new_plan(checkout, subscription),
            backend: backend.try_into()?,
        })
    }
}

/// The plan body for a self-serve checkout return or for a subscription to attach. The
/// clap `ArgGroup` requires exactly one of them.
fn new_plan(checkout: Option<NonEmpty>, subscription: Option<NonEmpty>) -> JsonNewPlan {
    match (checkout, subscription) {
        (Some(checkout), None) => JsonPlanCheckout {
            checkout: checkout.into(),
        }
        .into(),
        (None, Some(subscription)) => JsonPlanSubscription {
            subscription: subscription.into(),
        }
        .into(),
        #[expect(
            clippy::unreachable,
            reason = "clap ArgGroup requires exactly one of `checkout`/`subscription`"
        )]
        (Some(_), Some(_)) | (None, None) => {
            unreachable!("Exactly one of `checkout` or `subscription` is required")
        },
    }
}

impl SubCmd for Create {
    async fn exec(&self) -> Result<(), CliError> {
        let _json = self
            .backend
            .send(|client| async move {
                client
                    .org_plan_post()
                    .organization(self.organization.clone())
                    .body(self.plan.clone())
                    .send()
                    .await
            })
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::new_plan;

    // Fails if a checkout return is sent as an attach, an attach as a checkout return,
    // or either body carries anything else.
    #[test]
    fn new_plan_sends_the_one_given() {
        assert_eq!(
            serde_json::to_value(new_plan(Some("cs_test".parse().unwrap()), None)).unwrap(),
            serde_json::json!({ "checkout": "cs_test" })
        );
        assert_eq!(
            serde_json::to_value(new_plan(None, Some("sub_test".parse().unwrap()))).unwrap(),
            serde_json::json!({ "subscription": "sub_test" })
        );
    }
}
