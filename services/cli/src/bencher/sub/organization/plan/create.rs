use bencher_client::types::{JsonNewPlan, JsonPlanCheckout, JsonPlanSubscription};
use bencher_json::{NonEmpty, OrganizationResourceId, OrganizationUuid};

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
            self_hosted,
            backend,
        } = create;
        Ok(Self {
            organization,
            plan: new_plan(checkout, subscription, self_hosted),
            backend: backend.try_into()?,
        })
    }
}

/// The plan body for a self-serve checkout return or for a subscription to attach, which may
/// name the Self-Hosted organization it buys a license for. The clap `ArgGroup` requires
/// exactly one of a checkout or a subscription, and a Self-Hosted organization requires a
/// subscription.
fn new_plan(
    checkout: Option<NonEmpty>,
    subscription: Option<NonEmpty>,
    self_hosted: Option<OrganizationUuid>,
) -> JsonNewPlan {
    match (checkout, subscription) {
        (Some(checkout), None) => JsonPlanCheckout {
            checkout: checkout.into(),
        }
        .into(),
        (None, Some(subscription)) => JsonPlanSubscription {
            subscription: subscription.into(),
            self_hosted: self_hosted.map(Into::into),
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
    use bencher_json::OrganizationUuid;

    use super::new_plan;

    // Fails if a checkout return is sent as an attach, an attach as a checkout return,
    // either body carries anything else, or an attach for a Self-Hosted organization does not
    // name it.
    #[test]
    fn new_plan_sends_the_one_given() {
        assert_eq!(
            serde_json::to_value(new_plan(Some("cs_test".parse().unwrap()), None, None)).unwrap(),
            serde_json::json!({ "checkout": "cs_test" })
        );
        assert_eq!(
            serde_json::to_value(new_plan(None, Some("sub_test".parse().unwrap()), None)).unwrap(),
            serde_json::json!({ "subscription": "sub_test" })
        );
        let self_hosted = OrganizationUuid::new();
        assert_eq!(
            serde_json::to_value(new_plan(
                None,
                Some("sub_test".parse().unwrap()),
                Some(self_hosted)
            ))
            .unwrap(),
            serde_json::json!({ "subscription": "sub_test", "self_hosted": self_hosted })
        );
    }
}
